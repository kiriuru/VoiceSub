/** Self-contained Web Speech API surface for the Svelte worker (no DOM lib dependency). */

export interface WorkerSpeechRecognitionAlternative {
  readonly transcript: string;
  readonly confidence: number;
}

export interface WorkerSpeechRecognitionResult {
  readonly isFinal: boolean;
  readonly length: number;
  item(index: number): WorkerSpeechRecognitionAlternative;
  [index: number]: WorkerSpeechRecognitionAlternative;
}

export interface WorkerSpeechRecognitionResultList {
  readonly length: number;
  item(index: number): WorkerSpeechRecognitionResult;
  [index: number]: WorkerSpeechRecognitionResult;
}

export interface WorkerSpeechRecognitionEvent extends Event {
  readonly resultIndex: number;
  readonly results: WorkerSpeechRecognitionResultList;
}

export interface WorkerSpeechRecognitionErrorEvent extends Event {
  readonly error: string;
  readonly message: string;
}

export interface WorkerSpeechRecognition extends EventTarget {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  maxAlternatives: number;
  onstart: ((this: WorkerSpeechRecognition, ev: Event) => unknown) | null;
  onend: ((this: WorkerSpeechRecognition, ev: Event) => unknown) | null;
  onerror: ((this: WorkerSpeechRecognition, ev: WorkerSpeechRecognitionErrorEvent) => unknown) | null;
  onresult: ((this: WorkerSpeechRecognition, ev: WorkerSpeechRecognitionEvent) => unknown) | null;
  onsoundstart: ((this: WorkerSpeechRecognition, ev: Event) => unknown) | null;
  onsoundend: ((this: WorkerSpeechRecognition, ev: Event) => unknown) | null;
  onspeechstart: ((this: WorkerSpeechRecognition, ev: Event) => unknown) | null;
  onspeechend: ((this: WorkerSpeechRecognition, ev: Event) => unknown) | null;
  onaudiostart: ((this: WorkerSpeechRecognition, ev: Event) => unknown) | null;
  onaudioend: ((this: WorkerSpeechRecognition, ev: Event) => unknown) | null;
  start(): void;
  stop(): void;
  abort(): void;
}

export type SpeechRecognitionConstructor = {
  new (): WorkerSpeechRecognition;
};
