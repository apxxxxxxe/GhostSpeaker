[![GhostSpeaker.nar](https://img.shields.io/github/v/release/apxxxxxxe/GhostSpeaker?color=%238a4e4e&label=GhostSpeaker.nar&logo=github)](https://github.com/apxxxxxxe/GhostSpeaker/releases/latest/download/GhostSpeaker.nar)
[![commits](https://img.shields.io/github/last-commit/apxxxxxxe/GhostSpeaker?color=%238a4e4e&label=%E6%9C%80%E7%B5%82%E6%9B%B4%E6%96%B0&logo=github)](https://github.com/apxxxxxxe/GhostSpeaker/commits/main)

# 伺かプラグイン「GhostSpeaker」

https://github.com/apxxxxxxe/GhostSpeaker/assets/39634779/3de99b5d-5f54-4d77-83be-60c7cf055dc9

デモ動画（音声がミュートになっていないことを確認してください）

- SSPで動作確認

## 何をするもの？
音声合成エンジンを利用して、ゴーストの台詞を読み上げることができるプラグインです。  
現在対応している音声合成エンジンは、

- [COEIROINK(v1.x.x)](https://coeiroink.com/)
- [COEIROINK(v2.x.x)](https://coeiroink.com/)
- [ITVOICE](http://itvoice.starfree.jp/)
- [LMROID](https://lmroidsoftware.wixsite.com/nhoshio)
- [SHAREVOX](https://www.sharevox.app/)
- [VOICEVOX](https://voicevox.hiroshiba.jp/)
- [棒読みちゃん](https://chi.usamimi.info/Program/Application/BouyomiChan/)

です。

各エンジンは以下のバージョンで動作確認済みです。
| Engine       | Version  |
| ---------    | -------- |
| COEIROINK    | v1.3.0   | 
| COEIROINK    | v2.1.1   |
| ITVOICE      | v0.1.2   |
| LMROID       | v1.4.0   |
| SHAREVOX     | v0.2.1   |
| VOICEVOX     | v0.14.10 |
| 棒読みちゃん | 0.1.10.0 |

## どうやって使うの？
プラグインをインストール後、対応する音声合成エンジンを起動してください。例えば、VOICEVOXの場合は`VOICEVOX.exe`を起動します。

![image](https://github.com/apxxxxxxe/GhostSpeaker/assets/39634779/854f52ee-c1cf-4775-b7af-969f62abed87)  
エンジンの準備が完了すると、上図のような通知がされます。

![image](https://github.com/apxxxxxxe/GhostSpeaker/assets/39634779/01f09639-1b1c-451b-92f0-7e440bb85996)  
また、プラグイン実行時のメニューでエンジンが"起動中"となっていることを確認してください。

![image](https://github.com/apxxxxxxe/GhostSpeaker/assets/39634779/a7fae01f-1deb-4844-8e05-8141070f6c2f)  
メニューから、**起動中の**エンジンで利用可能な声質が選択可能です。  
- **デフォルトでは読み上げ声質は"無し"となっており、そのままでは読み上げられません。**
  - メニュー下部から「デフォルト声質(共通)」を設定することで解決が可能です。
- 起動中のエンジンがない場合、選択可能な声質はありません。

エンジンの準備が完了し次第、ゴーストの台詞が読み上げられるようになります。

## インストール方法
ゴーストのインストールと同様に、本プラグインのnarファイルを起動中のゴーストにドラッグ＆ドロップしてください。  

## ダウンロード
[![GhostSpeaker.nar](https://img.shields.io/github/v/release/apxxxxxxe/GhostSpeaker?color=%238a4e4e&label=GhostSpeaker.nar&logo=github)](https://github.com/apxxxxxxe/GhostSpeaker/releases/latest/download/GhostSpeaker.nar) 

## 設定項目

### 音量調整(共通)
読み上げ時の音量調整が可能です。  
ただし、現在は棒読みちゃんのみ非対応です。棒読みちゃん本体側の音量調節をお使いください。

### 句読点ごとに読み上げ(共通)
通常、読み上げ時はトーク全体をひとまとめにして音声合成を行います(本設定がオフ)が、
基本的には句読点で区切って一文ごとに合成する(本設定がオン)ほうが読み始めるまでの時間が短くなります。  
お好みで切り替えが可能です。

### 改行で一拍置く(ゴースト別)
ゴーストによっては、トークに句読点を使わず、改行のみで文を区切っているものがあります。  
しかし、GhostSpeakerでは改行は無視されるため、そのままでは各文が連結して読み上げられてしまいます。  
そこで本設定をオンにすることで改行を句読点とみなし、区切りながら読み上げさせることが可能です。

### 各音声合成エンジンの自動起動
![image](https://github.com/apxxxxxxe/GhostSpeaker/assets/39634779/5d0896b3-775b-4390-af27-911c49cab89d)

各音声合成エンジンについて、プラグイン起動時に自動で起動するかどうかを設定することができます。  
設定手順は以下のようになっています。
- 上述の手順で音声合成エンジンを接続する。
- 音声合成エンジンの実行ファイルのパスがプラグインに保存され、`設定未完了`だった設定項目が`無効`に変わります。
- `無効`ボタンをクリックして`有効`にすることで、次回以降の起動時に音声合成エンジンが同時に起動するようになります。

### 終了時に読み上げが終わるのを待つ(共通)
SSP終了時(厳密にはプラグインunload時)に  
- 最後まで読み上げられるのを待ってから終了する(`有効`)
- 読み上げを打ち切ってすぐに終了する(`無効`)

のどちらの挙動を採用するかを切り替えられます。

### デフォルト声質(共通)
ゴーストごとの声質が`未設定`の場合に使用する声質を指定します。  
これを指定することで、初回起動からゴーストのトークを読み上げることが可能になります。

## 更新履歴
各バージョンの更新内容は[こちら](https://github.com/apxxxxxxe/GhostSpeaker/releases)からご確認ください。

