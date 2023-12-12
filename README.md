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
プラグインをインストール後、対応する音声合成エンジンを起動してください。例えば、VOICEVOXの場合は`VOICEVOX.exe`を起動します。

![image](https://github.com/apxxxxxxe/GhostSpeaker/assets/39634779/839f5241-6e00-46b1-8d53-49dfecce10e2)    
エンジンの準備が完了すると、上図のような通知がされます。

![image](https://github.com/apxxxxxxe/GhostSpeaker/assets/39634779/480b81e6-9665-4577-9b53-c4c27b23c47c)  
また、プラグイン実行時のメニューでエンジンが"起動中"となっていることを確認してください。

![image](https://github.com/apxxxxxxe/GhostSpeaker/assets/39634779/d0f8e33b-4958-4c3d-9946-fad84d302fd8)  
**デフォルトでは読み上げ声質は"無し"となっており、そのままでは読み上げられません。**  
メニューから、**起動中の**エンジンで利用可能な声質が選択可能です。  
（起動中のエンジンがない場合、選択可能な声質はありません。）

エンジンの準備が完了し次第、ゴーストの台詞が読み上げられるようになります。

## インストール方法
ゴーストのインストールと同様に、本プラグインのnarファイルを起動中のゴーストにドラッグ＆ドロップしてください。  

## ダウンロード

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
