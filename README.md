# 伺かプラグイン「GhostSpeaker」

https://github.com/apxxxxxxe/GhostSpeaker/assets/39634779/5e13bf62-1c07-45c9-a5f8-d3ed043b24b0

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
プラグインをインストール後、対応する音声合成エンジンを起動してください。例えば、COEIROINKの場合は`COEIROINKv2.exe`もしくは`engine/engine.exe`を起動します。

![image](https://github.com/apxxxxxxe/GhostSpeaker/assets/39634779/839f5241-6e00-46b1-8d53-49dfecce10e2)    
エンジンの準備が完了すると、上図のような通知がされます。

![image](https://github.com/apxxxxxxe/GhostSpeaker/assets/39634779/480b81e6-9665-4577-9b53-c4c27b23c47c)  
また、プラグイン実行時のメニューでエンジンが"起動中"となっていることを確認してください。

![image](https://github.com/apxxxxxxe/GhostSpeaker/assets/39634779/d0f8e33b-4958-4c3d-9946-fad84d302fd8)  
デフォルトでは読み上げ声質は"無し"となっており、そのままでは読み上げられません。
メニューから、**起動中の**エンジンで利用可能な声質が選択可能です。  
（起動中のエンジンがない場合、選択可能な声質はありません。）

エンジンの準備が完了し次第、ゴーストの台詞が読み上げられるようになります。

## インストール方法
ゴーストのインストールと同様に、本プラグインのnarファイルを起動中のゴーストにドラッグ＆ドロップしてください。  

## 注意
インストール直後はバージョンが古い場合があるため、必ずネットワーク更新を行ってください。  
本プラグインの右クリックメニューからネットワーク更新が可能です。

## ダウンロード
