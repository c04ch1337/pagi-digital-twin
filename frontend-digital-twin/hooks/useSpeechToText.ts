import * as React from 'react';

type SpeechRecognitionLike = {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  maxAlternatives: number;
  onstart: (() => void) | null;
  onend: (() => void) | null;
  onerror: ((e: any) => void) | null;
  onresult: ((e: any) => void) | null;
  start: () => void;
  stop: () => void;
  abort: () => void;
};

export type SpeechToTextState = {
  isSupported: boolean;
  isListening: boolean;
  finalText: string;
  interimText: string;
  error: string | null;
};

export type UseSpeechToTextOptions = {
  lang?: string;
  continuous?: boolean;
  interimResults?: boolean;
};

function normalizeWhitespace(s: string): string {
  return s.replace(/\s+/g, ' ').trim();
}

export function useSpeechToText(options: UseSpeechToTextOptions = {}) {
  const lang = options.lang ?? 'en-US';
  const continuous = options.continuous ?? false;
  const interimResults = options.interimResults ?? true;

  const recognitionRef = React.useRef<SpeechRecognitionLike | null>(null);

  const [isSupported, setIsSupported] = React.useState(false);
  const [isListening, setIsListening] = React.useState(false);
  const [finalText, setFinalText] = React.useState('');
  const [interimText, setInterimText] = React.useState('');
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    const w = window as any;
    const Ctor = w.SpeechRecognition || w.webkitSpeechRecognition;
    if (!Ctor) {
      setIsSupported(false);
      return;
    }
    setIsSupported(true);

    const rec: SpeechRecognitionLike = new Ctor();
    rec.lang = lang;
    rec.continuous = continuous;
    rec.interimResults = interimResults;
    rec.maxAlternatives = 1;

    rec.onstart = () => {
      setIsListening(true);
      setError(null);
    };
    rec.onend = () => {
      setIsListening(false);
      setInterimText('');
    };
    rec.onerror = (e: any) => {
      const msg = normalizeWhitespace(String(e?.error || e?.message || 'Speech recognition error'));
      setError(msg);
      setIsListening(false);
      setInterimText('');
    };

    rec.onresult = (e: any) => {
      try {
        let nextFinal = '';
        let nextInterim = '';
        const results = e?.results;
        if (results && typeof results.length === 'number') {
          for (let i = e.resultIndex ?? 0; i < results.length; i += 1) {
            const r = results[i];
            const text = normalizeWhitespace(String(r?.[0]?.transcript ?? ''));
            if (!text) continue;
            if (r?.isFinal) nextFinal = normalizeWhitespace([nextFinal, text].filter(Boolean).join(' '));
            else nextInterim = normalizeWhitespace([nextInterim, text].filter(Boolean).join(' '));
          }
        }

        if (nextFinal) {
          setFinalText((prev) => normalizeWhitespace([prev, nextFinal].filter(Boolean).join(' ')));
        }
        setInterimText(nextInterim);
      } catch (err) {
        console.warn('[useSpeechToText] Failed to parse result', err);
      }
    };

    recognitionRef.current = rec;

    return () => {
      try {
        rec.onstart = null;
        rec.onend = null;
        rec.onerror = null;
        rec.onresult = null;
        rec.abort();
      } catch {
        // ignore
      }
      recognitionRef.current = null;
    };
  }, [lang, continuous, interimResults]);

  const reset = React.useCallback(() => {
    setFinalText('');
    setInterimText('');
    setError(null);
  }, []);

  const start = React.useCallback(() => {
    const rec = recognitionRef.current;
    if (!rec) {
      setError('Speech recognition not supported in this browser.');
      return;
    }
    setError(null);
    setFinalText('');
    setInterimText('');
    try {
      rec.start();
    } catch (e) {
      // Some browsers throw if started twice
      const msg = normalizeWhitespace(String((e as any)?.message || 'Failed to start speech recognition'));
      setError(msg);
    }
  }, []);

  const stop = React.useCallback(() => {
    const rec = recognitionRef.current;
    if (!rec) return;
    try {
      rec.stop();
    } catch {
      // ignore
    }
  }, []);

  const state: SpeechToTextState = {
    isSupported,
    isListening,
    finalText,
    interimText,
    error,
  };

  return {
    state,
    actions: {
      start,
      stop,
      reset,
    },
  };
}

