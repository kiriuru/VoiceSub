/// <reference lib="dom" />

interface Window {
  SpeechRecognition?: import("./lib/asr/types").SpeechRecognitionConstructor;
  webkitSpeechRecognition?: import("./lib/asr/types").SpeechRecognitionConstructor;
}
