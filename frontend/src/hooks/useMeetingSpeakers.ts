import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import type { AssignSpeakerResult, MeetingSpeaker, PersonSummary, Transcript } from '@/types';

/** Who a speaker should be named: an existing person, or a name typed for the first time. */
export type SpeakerTarget = { personId: string } | { newName: string } | null;

export interface MeetingSpeakersApi {
  meetingId: string;
  speakers: MeetingSpeaker[];
  /** Everyone in the directory, for the "who is this?" dropdown. */
  people: PersonSummary[];
  /** Name a speaker (null clears the name). Every line from that speaker follows. */
  nameSpeaker: (speakerKey: string, target: SpeakerTarget) => Promise<boolean>;
  /** Move one line to another speaker (null leaves it unlabelled). */
  moveLine: (transcriptId: string, speakerKey: string | null) => Promise<boolean>;
  /** Add the next free speaker slot and return its key. */
  addSpeaker: () => Promise<string | null>;
  /** Fold one speaker into another (used when one person was split in two). */
  mergeSpeakers: (fromKey: string, intoKey: string) => Promise<boolean>;
  /** Remove a speaker; its lines become unlabelled. */
  removeSpeaker: (speakerKey: string) => Promise<boolean>;
}

type PatchTranscripts = (update: (transcript: Transcript) => Transcript) => void;

/**
 * Speaker slots and the people directory for one meeting.
 *
 * Lines only carry a slot key ("S1"). Naming a slot re-labels all of its lines at once
 * because the display name is looked up from `speakers`, not stored on each line.
 * `patchTranscripts` keeps the already-loaded lines in step with line moves and merges.
 */
export function useMeetingSpeakers(
  meetingId: string,
  patchTranscripts?: PatchTranscripts,
): MeetingSpeakersApi {
  const [speakers, setSpeakers] = useState<MeetingSpeaker[]>([]);
  const [people, setPeople] = useState<PersonSummary[]>([]);
  const activeMeetingRef = useRef(meetingId);
  activeMeetingRef.current = meetingId;

  const refreshPeople = useCallback(async () => {
    try {
      setPeople(await invoke<PersonSummary[]>('api_list_people', { query: null }));
    } catch (error) {
      console.error('Failed to load people:', error);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    setSpeakers([]);
    (async () => {
      try {
        const loaded = await invoke<MeetingSpeaker[]>('api_get_meeting_speakers', { meetingId });
        if (!cancelled) setSpeakers(loaded);
      } catch (error) {
        console.error('Failed to load speakers:', error);
      }
    })();
    void refreshPeople();
    return () => {
      cancelled = true;
    };
  }, [meetingId, refreshPeople]);

  // Run a backend call that returns the meeting's fresh speaker list.
  const run = useCallback(
    async (action: () => Promise<MeetingSpeaker[]>, failure: string): Promise<boolean> => {
      try {
        const updated = await action();
        if (activeMeetingRef.current === meetingId) setSpeakers(updated);
        return true;
      } catch (error) {
        toast.error(failure, { description: String(error) });
        return false;
      }
    },
    [meetingId],
  );

  const nameSpeaker = useCallback(
    async (speakerKey: string, target: SpeakerTarget) => {
      try {
        const result = await invoke<AssignSpeakerResult>('api_assign_speaker_person', {
          meetingId,
          speakerKey,
          personId: target && 'personId' in target ? target.personId : null,
          newPersonName: target && 'newName' in target ? target.newName : null,
        });
        if (activeMeetingRef.current === meetingId) setSpeakers(result.speakers);
        // A new name joins the dropdown immediately; counts for others refresh too.
        await refreshPeople();
        return true;
      } catch (error) {
        toast.error('Could not name this speaker', { description: String(error) });
        return false;
      }
    },
    [meetingId, refreshPeople],
  );

  const moveLine = useCallback(
    async (transcriptId: string, speakerKey: string | null) => {
      const ok = await run(
        () => invoke<MeetingSpeaker[]>('api_set_transcript_speaker', { transcriptId, speakerKey }),
        'Could not change the speaker for this line',
      );
      if (ok) {
        patchTranscripts?.((t) =>
          t.id === transcriptId ? { ...t, speaker: speakerKey ?? undefined } : t,
        );
      }
      return ok;
    },
    [run, patchTranscripts],
  );

  const addSpeaker = useCallback(async () => {
    try {
      const before = new Set(speakers.map((s) => s.speaker_key));
      const updated = await invoke<MeetingSpeaker[]>('api_add_meeting_speaker', { meetingId });
      if (activeMeetingRef.current === meetingId) setSpeakers(updated);
      return updated.find((s) => !before.has(s.speaker_key))?.speaker_key ?? null;
    } catch (error) {
      toast.error('Could not add a speaker', { description: String(error) });
      return null;
    }
  }, [meetingId, speakers]);

  const mergeSpeakers = useCallback(
    async (fromKey: string, intoKey: string) => {
      const ok = await run(
        () =>
          invoke<MeetingSpeaker[]>('api_merge_meeting_speakers', {
            meetingId,
            fromKey,
            intoKey,
          }),
        'Could not merge these speakers',
      );
      if (ok) {
        patchTranscripts?.((t) => (t.speaker === fromKey ? { ...t, speaker: intoKey } : t));
      }
      return ok;
    },
    [meetingId, run, patchTranscripts],
  );

  const removeSpeaker = useCallback(
    async (speakerKey: string) => {
      const ok = await run(
        () => invoke<MeetingSpeaker[]>('api_delete_meeting_speaker', { meetingId, speakerKey }),
        'Could not remove this speaker',
      );
      if (ok) {
        patchTranscripts?.((t) => (t.speaker === speakerKey ? { ...t, speaker: undefined } : t));
      }
      return ok;
    },
    [meetingId, run, patchTranscripts],
  );

  return { meetingId, speakers, people, nameSpeaker, moveLine, addSpeaker, mergeSpeakers, removeSpeaker };
}
