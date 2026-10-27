import { invoke } from '@tauri-apps/api/core';
import type { MeetingSpeaker } from '@/types';

/** "S3" -> 3, anything else -> null */
export function speakerNumber(key: string): number | null {
  const match = /^S([1-9]\d*)$/.exec(key);
  return match ? Number(match[1]) : null;
}

/** The name shown for a speaker: their person's name, or "Speaker N" until named. */
export function speakerDisplayName(key: string, speaker?: Pick<MeetingSpeaker, 'person_name'>): string {
  if (speaker?.person_name) return speaker.person_name;
  const n = speakerNumber(key);
  return n ? `Speaker ${n}` : key;
}

/** key -> display name for every speaker in the list. */
export function speakerNameMap(speakers: MeetingSpeaker[]): Record<string, string> {
  const names: Record<string, string> = {};
  for (const s of speakers) names[s.speaker_key] = speakerDisplayName(s.speaker_key, s);
  return names;
}

/**
 * "Maj Smith: " for a labelled line, "" for an unlabelled one, so meetings without
 * speakers produce exactly the same text as before.
 */
export function speakerPrefix(key: string | undefined | null, names: Record<string, string>): string {
  if (!key) return '';
  return `${names[key] ?? speakerDisplayName(key)}: `;
}

/** Fresh names straight from the database (used when copying or summarising). */
export async function fetchSpeakerNames(meetingId: string): Promise<Record<string, string>> {
  try {
    const speakers = await invoke<MeetingSpeaker[]>('api_get_meeting_speakers', { meetingId });
    return speakerNameMap(speakers);
  } catch (error) {
    // Names are a nicety; never block a copy or a summary on them.
    console.error('Failed to load speaker names:', error);
    return {};
  }
}

// A fixed palette so a speaker keeps the same colour everywhere in a meeting.
const SPEAKER_COLORS = [
  'bg-blue-50 text-blue-700 border-blue-200',
  'bg-emerald-50 text-emerald-700 border-emerald-200',
  'bg-amber-50 text-amber-800 border-amber-200',
  'bg-purple-50 text-purple-700 border-purple-200',
  'bg-rose-50 text-rose-700 border-rose-200',
  'bg-cyan-50 text-cyan-700 border-cyan-200',
  'bg-orange-50 text-orange-700 border-orange-200',
  'bg-indigo-50 text-indigo-700 border-indigo-200',
];

export function speakerColorClasses(key: string): string {
  const n = speakerNumber(key) ?? 1;
  return SPEAKER_COLORS[(n - 1) % SPEAKER_COLORS.length];
}
