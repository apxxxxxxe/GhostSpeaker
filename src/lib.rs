mod engine;
mod events;
mod format;
mod player;
mod plugin;
mod queue;
mod speaker;
mod utils;
mod variables;

use crate::engine::coeiroink;
use crate::engine::voicevox;
use crate::plugin::request::PluginRequest;
use crate::queue::get_queue;
use crate::variables::get_global_vars;

use shiori_hglobal::*;
use shiorust::message::Parser;
use simplelog::*;
use std::fs::File;
use std::panic;
use std::path::Path;
use winapi::ctypes::c_long;
use winapi::shared::minwindef::{BOOL, HGLOBAL, TRUE};

#[macro_use]
extern crate log;
extern crate simplelog;

pub static mut DLL_PATH: String = String::new();

#[derive(Debug)]
pub enum ResponseError {
  DecodeFailed,
}

#[no_mangle]
pub extern "cdecl" fn load(h: HGLOBAL, len: c_long) -> BOOL {
  let v = GStr::capture(h, len as usize);
  let s = v.to_utf8_str().unwrap();

  get_global_vars().volatility.dll_dir = s.to_string();
  get_global_vars().load();

  let log_path = Path::new(&get_global_vars().volatility.dll_dir).join("ghost-speaker.log");
  WriteLogger::init(
    LevelFilter::Debug,
    Config::default(),
    File::create(log_path).unwrap(),
  )
  .unwrap();

  panic::set_hook(Box::new(|panic_info| {
    debug!("{}", panic_info);
  }));

  coeiroink::speaker::get_speaker_getter().start();
  voicevox::speaker::get_speaker_getter().start();

  debug!("load");

  return TRUE;
}

#[no_mangle]
pub extern "cdecl" fn unload() -> BOOL {
  get_global_vars().save();
  get_queue().stop();
  coeiroink::speaker::get_speaker_getter().stop();
  voicevox::speaker::get_speaker_getter().stop();

  debug!("unload");

  return TRUE;
}

#[no_mangle]
pub extern "cdecl" fn request(h: HGLOBAL, len: *mut c_long) -> HGLOBAL {
  // リクエストの取得
  let v = unsafe { GStr::capture(h, *len as usize) };

  let s = v.to_utf8_str().unwrap();

  let pr = PluginRequest::parse(&s).unwrap();
  let r = pr.request;

  let response = events::handle_request(&r);

  let bytes = response.to_string().into_bytes();
  let response_gstr = GStr::clone_from_slice_nofree(&bytes);

  unsafe { *len = response_gstr.len() as c_long };
  response_gstr.handle()
}

#[cfg(test)]
mod test {
  use crate::engine::ENGINE_COEIROINK;
  use crate::events::handle_request;
  use crate::plugin::request::PluginRequest;
  use crate::queue::get_queue;
  use crate::utils::check_connection;
  use crate::variables::get_global_vars;
  use shiorust::message::Parser;
  use std::time::Duration;

  #[test]
  fn test_main() {
    get_global_vars().load();

    // init
    get_queue();

    futures::executor::block_on(async {
      while !check_connection(ENGINE_COEIROINK).await {
        println!("waiting...");
        std::thread::sleep(Duration::from_secs(1));
      }
    });

    let pr = PluginRequest::parse(
            "\
            GET PLUGIN/2.0\r\n\
            ID: OnOtherGhostTalk\r\n\
            Charset: UTF-8\r\n\
            Reference0: Test\r\n\
            Reference1: dummy\r\n\
            Reference2: dummy\r\n\
            Reference3: dummy\r\n\
            Reference4: \\0こんにちは\\1ゴーストはこちらを見て微笑んだ。\\0あれ、なにか顔についてる？\\n違う？ならいいけど。\r\n\
            \r\n\
            ",
        )
        .unwrap();
    let r = pr.request;
    println!("{:?}", r);

    let res = handle_request(&r);
    println!("{:?}", res);

    let pr = PluginRequest::parse(
      "\
            GET PLUGIN/2.0\r\n\
            ID: OnOtherGhostTalk\r\n\
            Charset: UTF-8\r\n\
            Reference0: Test\r\n\
            Reference1: dummy\r\n\
            Reference2: dummy\r\n\
            Reference3: dummy\r\n\
            Reference4: \\0第2トーク。\\1ゴーストは本に目を通している。\r\n\
            \r\n\
            ",
    )
    .unwrap();
    let r = pr.request;
    println!("{:?}", r);

    let res = handle_request(&r);
    println!("{:?}", res);

    for i in 0..20 {
      println!("{}", i);
      std::thread::sleep(Duration::from_secs(1));
    }
  }
}
