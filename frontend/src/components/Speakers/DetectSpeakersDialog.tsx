'use client';

import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { AlertCircle, CheckCircle2, Download, Loader2, Users } from 'lucide-react';
import { Dialog, DialogContent, DialogFooter, DialogTitle } from '@/components/ui/dialog';
import { VisuallyHidden } from '@/components/ui/visually-hidden';
import type { DetectSpeakersResult, SpeakerModelStatus } from '@/types';

interface DetectSpeakersDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  meetingId: string;
  /** False when the meeting has no saved recording to analyse. */
  hasRecording: boolean;
  /** True when speakers already exist, so detection will replace them. */
  hasSpeakers: boolean;
  /** Called after detection saved new speakers, so the page can refresh. */
  onDetected: () => Promise<void>;
}

type Phase = 'idle' | 'running' | 'done' | 'error';

interface DetectionProgress {
  meeting_id: string;
  phase: 'decoding' | 'analyzing' | 'clustering';
  done: number;
  total: number;
}

const COUNT_OPTIONS = [1, 2, 3, 4, 5, 6, 7, 8];

const megabytes = (bytes: number) => (bytes / (1024 * 1024)).toFixed(0);

function ProgressBar({ fraction }: { fraction: number | null }) {
  return (
    <div className="h-2 w-full rounded-full bg-gray-100 overflow-hidden" role="progressbar">
      <div
        className={`h-full bg-blue-500 transition-all duration-200 ${fraction === null ? 'w-1/3 animate-pulse' : ''}`}
        style={fraction === null ? undefined : { width: `${Math.round(fraction * 100)}%` }}
      />
    </div>
  );
}

/** Find who spoke each line by comparing voices in the recording. Runs entirely on this computer. */
export function DetectSpeakersDialog({
  open,
  onOpenChange,
  meetingId,
  hasRecording,
  hasSpeakers,
  onDetected,
}: DetectSpeakersDialogProps) {
  const [model, setModel] = useState<SpeakerModelStatus | null>(null);
  const [isDownloading, setIsDownloading] = useState(false);
  const [download, setDownload] = useState<{ downloaded: number; total: number } | null>(null);
  const [downloadError, setDownloadError] = useState<string | null>(null);

  const [count, setCount] = useState<string>('auto');
  const [phase, setPhase] = useState<Phase>('idle');
  const [progress, setProgress] = useState<DetectionProgress | null>(null);
  const [result, setResult] = useState<DetectSpeakersResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Start fresh each time the dialog opens
  useEffect(() => {
    if (!open) return;
    setPhase('idle');
    setProgress(null);
    setResult(null);
    setError(null);
    setDownloadError(null);
    (async () => {
      try {
        setModel(await invoke<SpeakerModelStatus>('speaker_model_status'));
      } catch (e) {
        setDownloadError(String(e));
      }
    })();
  }, [open]);

  // Progress events from the backend while the dialog is open
  useEffect(() => {
    if (!open) return;
    let disposed = false;
    const unlisteners: UnlistenFn[] = [];
    (async () => {
      const handlers = [
        await listen<DetectionProgress>('speaker-detection-progress', (event) => {
          if (event.payload.meeting_id === meetingId) setProgress(event.payload);
        }),
        await listen<{ downloaded: number; total: number }>('speaker-model-download-progress', (event) =>
          setDownload(event.payload),
        ),
      ];
      if (disposed) handlers.forEach((unlisten) => unlisten());
      else unlisteners.push(...handlers);
    })();
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [open, meetingId]);

  const downloadModel = async () => {
    setIsDownloading(true);
    setDownloadError(null);
    setDownload(null);
    try {
      await invoke('speaker_model_download');
      setModel(await invoke<SpeakerModelStatus>('speaker_model_status'));
    } catch (e) {
      setDownloadError(String(e));
    } finally {
      setIsDownloading(false);
    }
  };

  const detect = async () => {
    setPhase('running');
    setProgress(null);
    setError(null);
    try {
      const detected = await invoke<DetectSpeakersResult>('api_detect_speakers', {
        meetingId,
        numSpeakers: count === 'auto' ? null : Number(count),
      });
      setResult(detected);
      setPhase('done');
      await onDetected();
    } catch (e) {
      setError(String(e));
      setPhase('error');
    }
  };

  const isRunning = phase === 'running';
  const modelReady = model?.downloaded === true;
  const analyzeFraction = progress?.phase === 'analyzing' && progress.total > 0 ? progress.done / progress.total : null;

  return (
    <Dialog open={open} onOpenChange={(next) => { if (!isRunning) onOpenChange(next); }}>
      <DialogContent className="sm:max-w-[480px]">
        <VisuallyHidden>
          <DialogTitle>Detect speakers</DialogTitle>
        </VisuallyHidden>
        <div className="py-2 space-y-4">
          <div>
            <h3 className="text-lg font-semibold flex items-center">
              <Users className="w-5 h-5 mr-2 text-gray-500" />
              Detect speakers
            </h3>
            <p className="text-sm text-gray-600 mt-1">
              Works out who spoke each line by comparing the voices in the recording. It runs on this
              computer and nothing is uploaded.
            </p>
          </div>

          {!hasRecording && (
            <p className="text-sm text-amber-700 bg-amber-50 border border-amber-200 rounded-md p-3">
              This meeting has no saved recording, so there are no voices to analyse.
            </p>
          )}

          {hasRecording && model && !modelReady && phase === 'idle' && (
            <div className="border border-gray-200 rounded-md p-3 space-y-2">
              <p className="text-sm text-gray-700">
                First, a one-time download of the voice model ({megabytes(model.size_bytes)} MB). After that,
                detection works offline.
              </p>
              {isDownloading ? (
                <>
                  <ProgressBar fraction={download && download.total > 0 ? download.downloaded / download.total : null} />
                  <p className="text-xs text-gray-500">
                    {download
                      ? `Downloading… ${megabytes(download.downloaded)} of ${megabytes(download.total)} MB`
                      : 'Starting download…'}
                  </p>
                </>
              ) : (
                <button
                  onClick={() => void downloadModel()}
                  className="inline-flex items-center px-3 py-1.5 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-md"
                >
                  <Download className="w-4 h-4 mr-2" />
                  Download voice model
                </button>
              )}
              {downloadError && <p className="text-sm text-red-600">{downloadError}</p>}
            </div>
          )}

          {hasRecording && modelReady && phase !== 'done' && (
            <div className="space-y-3">
              <div>
                <label htmlFor="speaker-count" className="block text-sm font-medium text-gray-700 mb-1">
                  How many people spoke?
                </label>
                <select
                  id="speaker-count"
                  value={count}
                  disabled={isRunning}
                  onChange={(e) => setCount(e.target.value)}
                  className="w-full px-3 py-2 text-sm border border-gray-300 rounded-md bg-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                >
                  <option value="auto">Not sure (estimate it)</option>
                  {COUNT_OPTIONS.map((n) => (
                    <option key={n} value={n}>
                      {n} {n === 1 ? 'person' : 'people'}
                    </option>
                  ))}
                </select>
                <p className="text-xs text-gray-500 mt-1">
                  If you know the number, choose it: that is more accurate than estimating. Similar voices can be
                  mistaken for one person.
                </p>
              </div>
              {hasSpeakers && (
                <p className="text-xs text-gray-600 bg-gray-50 border border-gray-200 rounded-md p-2">
                  This replaces the current speaker labels. Names you have already given stay with the voices that
                  still match.
                </p>
              )}
            </div>
          )}

          {isRunning && (
            <div className="space-y-2" aria-live="polite">
              <ProgressBar fraction={analyzeFraction} />
              <p className="text-sm text-gray-600 flex items-center">
                <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                {progress?.phase === 'analyzing'
                  ? `Listening to voices… ${progress.done} of ${progress.total} lines`
                  : progress?.phase === 'clustering'
                    ? 'Grouping voices…'
                    : 'Reading the recording…'}
              </p>
            </div>
          )}

          {phase === 'done' && result && (
            <div className="flex items-start gap-2 text-sm text-green-800 bg-green-50 border border-green-200 rounded-md p-3">
              <CheckCircle2 className="w-4 h-4 mt-0.5 flex-shrink-0" />
              <div>
                Found {result.speakers.length} {result.speakers.length === 1 ? 'speaker' : 'speakers'} across{' '}
                {result.lines_labeled} lines. Click a speaker label in the transcript to give them a name. If two
                people were mixed up, run it again and set the number of people.
              </div>
            </div>
          )}

          {phase === 'error' && error && (
            <div className="flex items-start gap-2 text-sm text-red-700 bg-red-50 border border-red-200 rounded-md p-3">
              <AlertCircle className="w-4 h-4 mt-0.5 flex-shrink-0" />
              <div>{error}</div>
            </div>
          )}
        </div>
        <DialogFooter>
          {phase === 'done' ? (
            <button
              onClick={() => onOpenChange(false)}
              className="px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-md"
            >
              Done
            </button>
          ) : (
            <>
              <button
                onClick={() => onOpenChange(false)}
                disabled={isRunning}
                className="px-4 py-2 text-sm font-medium text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-md disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                onClick={() => void detect()}
                disabled={!hasRecording || !modelReady || isRunning}
                className="px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-md disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {phase === 'error' ? 'Try again' : 'Detect speakers'}
              </button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
