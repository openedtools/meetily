'use client';

import React, { useCallback, useEffect, useState } from 'react';
import { useRouter, useSearchParams } from 'next/navigation';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { Check, Merge, Pencil, Search, Trash2, User, Users, X } from 'lucide-react';
import { cn } from '@/lib/utils';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from '@/components/ui/command';
import { Dialog, DialogContent, DialogFooter, DialogTitle } from '@/components/ui/dialog';
import { VisuallyHidden } from '@/components/ui/visually-hidden';
import type { Person, PersonDetail, PersonLine, PersonSummary } from '@/types';

const LINE_LIMIT = 100;

function useDebounced<T>(value: T, ms: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), ms);
    return () => clearTimeout(timer);
  }, [value, ms]);
  return debounced;
}

function formatDate(iso: string | null | undefined): string {
  if (!iso) return '';
  const date = new Date(iso);
  return Number.isNaN(date.getTime())
    ? ''
    : date.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
}

function formatClock(seconds: number | null | undefined): string {
  if (seconds == null) return '';
  const total = Math.floor(seconds);
  return `${Math.floor(total / 60).toString().padStart(2, '0')}:${(total % 60).toString().padStart(2, '0')}`;
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

/** Bold the parts of `text` that match `query` (any script, case-insensitive). */
function Highlight({ text, query }: { text: string; query: string }) {
  const q = query.trim();
  if (!q) return <>{text}</>;
  const escaped = q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const parts = text.split(new RegExp(`(${escaped})`, 'giu'));
  return (
    <>
      {parts.map((part, i) =>
        i % 2 === 1 ? (
          <mark key={i} className="bg-yellow-100 rounded px-0.5">{part}</mark>
        ) : (
          <span key={i}>{part}</span>
        ),
      )}
    </>
  );
}

function ConfirmDialog({
  title,
  body,
  confirmLabel,
  destructive,
  onConfirm,
  onClose,
}: {
  title: string;
  body: string;
  confirmLabel: string;
  destructive?: boolean;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent className="sm:max-w-[425px]">
        <VisuallyHidden>
          <DialogTitle>{title}</DialogTitle>
        </VisuallyHidden>
        <div className="py-4">
          <h3 className="text-lg font-semibold mb-3">{title}</h3>
          <p className="text-sm text-gray-600">{body}</p>
        </div>
        <DialogFooter>
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm font-medium text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-md transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            className={cn(
              'px-4 py-2 text-sm font-medium text-white rounded-md transition-colors',
              destructive ? 'bg-red-600 hover:bg-red-700' : 'bg-blue-600 hover:bg-blue-700',
            )}
          >
            {confirmLabel}
          </button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** Everyone you have named, with a detail panel for the selected person. */
export function PeopleView() {
  const router = useRouter();
  const selectedId = useSearchParams().get('id');
  const [people, setPeople] = useState<PersonSummary[]>([]);
  const [isLoaded, setIsLoaded] = useState(false);
  const [filter, setFilter] = useState('');
  const debouncedFilter = useDebounced(filter, 200);

  const loadPeople = useCallback(async () => {
    try {
      setPeople(await invoke<PersonSummary[]>('api_list_people', { query: debouncedFilter.trim() || null }));
    } catch (error) {
      toast.error('Could not load people', { description: String(error) });
    } finally {
      setIsLoaded(true);
    }
  }, [debouncedFilter]);

  useEffect(() => {
    void loadPeople();
  }, [loadPeople]);

  const select = (id: string | null) => router.replace(id ? `/people?id=${encodeURIComponent(id)}` : '/people');

  return (
    <div className="flex h-screen min-w-0 bg-gray-50">
      {/* People list */}
      <div className="w-80 flex-shrink-0 border-r border-gray-200 bg-white flex flex-col">
        <div className="p-4 border-b border-gray-200">
          <h1 className="text-xl font-semibold text-gray-800 flex items-center">
            <Users className="w-5 h-5 mr-2 text-gray-500" />
            People
          </h1>
          <div className="relative mt-3">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-gray-400" />
            <input
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder="Search by name…"
              className="w-full pl-8 pr-3 py-2 text-sm border border-gray-200 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>
        </div>
        <div className="flex-1 overflow-y-auto">
          {isLoaded && people.length === 0 && (
            <p className="p-4 text-sm text-gray-500">
              {filter.trim()
                ? 'No one matches that name.'
                : 'No one here yet. Open a meeting, click a speaker label and give them a name; they will show up here.'}
            </p>
          )}
          {people.map((person) => (
            <button
              key={person.id}
              onClick={() => select(person.id)}
              className={cn(
                'w-full text-left px-4 py-3 border-b border-gray-100 hover:bg-gray-50 transition-colors',
                person.id === selectedId && 'bg-blue-50 hover:bg-blue-50',
              )}
            >
              <div className="font-medium text-gray-800 truncate">{person.name}</div>
              <div className="text-xs text-gray-500 mt-0.5">
                {person.meeting_count === 0
                  ? 'Not in any meeting yet'
                  : `${plural(person.meeting_count, 'meeting')} · last ${formatDate(person.last_meeting_at)}`}
              </div>
            </button>
          ))}
        </div>
      </div>

      {/* Detail */}
      <div className="flex-1 min-w-0 overflow-y-auto">
        {selectedId ? (
          <PersonPanel
            key={selectedId}
            personId={selectedId}
            allPeople={people}
            onChanged={loadPeople}
            onSelect={select}
          />
        ) : (
          <div className="h-full flex flex-col items-center justify-center text-center text-gray-500 px-6">
            <User className="w-10 h-10 mb-3 text-gray-300" />
            <p className="font-medium text-gray-600">Select a person</p>
            <p className="text-sm mt-1 max-w-sm">
              See every meeting you have had with them, search what they said, and keep notes.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

function PersonPanel({
  personId,
  allPeople,
  onChanged,
  onSelect,
}: {
  personId: string;
  allPeople: PersonSummary[];
  onChanged: () => Promise<void>;
  onSelect: (id: string | null) => void;
}) {
  const router = useRouter();
  const { setCurrentMeeting } = useSidebar();
  const [detail, setDetail] = useState<PersonDetail | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [isRenaming, setIsRenaming] = useState(false);
  const [nameDraft, setNameDraft] = useState('');

  const [notes, setNotes] = useState('');
  const [savedNotes, setSavedNotes] = useState('');
  const [notesState, setNotesState] = useState<'idle' | 'saving' | 'saved'>('idle');

  const [lineQuery, setLineQuery] = useState('');
  const debouncedLineQuery = useDebounced(lineQuery, 250);
  const [lines, setLines] = useState<PersonLine[]>([]);

  const [mergeOpen, setMergeOpen] = useState(false);
  const [confirm, setConfirm] = useState<{ kind: 'merge'; target: PersonSummary } | { kind: 'delete' } | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const loaded = await invoke<PersonDetail>('api_get_person', { personId });
        if (cancelled) return;
        setDetail(loaded);
        setNotes(loaded.person.notes ?? '');
        setSavedNotes(loaded.person.notes ?? '');
      } catch (error) {
        if (!cancelled) setLoadError(String(error));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [personId]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const found = await invoke<PersonLine[]>('api_get_person_lines', {
          personId,
          query: debouncedLineQuery.trim() || null,
          limit: LINE_LIMIT,
        });
        if (!cancelled) setLines(found);
      } catch (error) {
        console.error('Failed to load lines:', error);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [personId, debouncedLineQuery]);

  const openMeeting = (meetingId: string, title: string) => {
    setCurrentMeeting({ id: meetingId, title });
    router.push(`/meeting-details?id=${meetingId}`);
  };

  const saveName = async () => {
    const name = nameDraft.trim();
    if (!detail || !name || name === detail.person.name) {
      setIsRenaming(false);
      return;
    }
    try {
      const person = await invoke<Person>('api_rename_person', { personId, name });
      setDetail({ ...detail, person });
      setIsRenaming(false);
      await onChanged();
    } catch (error) {
      toast.error('Could not rename', { description: String(error) });
    }
  };

  const saveNotes = async () => {
    if (notes === savedNotes) return;
    setNotesState('saving');
    try {
      const person = await invoke<Person>('api_update_person_notes', { personId, notes: notes.trim() || null });
      setSavedNotes(person.notes ?? '');
      setNotes(person.notes ?? '');
      setNotesState('saved');
    } catch (error) {
      setNotesState('idle');
      toast.error('Could not save notes', { description: String(error) });
    }
  };

  const doMerge = async (target: PersonSummary) => {
    setConfirm(null);
    try {
      await invoke<Person>('api_merge_people', { fromId: personId, intoId: target.id });
      toast.success(`Merged into ${target.name}`);
      await onChanged();
      onSelect(target.id);
    } catch (error) {
      toast.error('Could not merge', { description: String(error) });
    }
  };

  const doDelete = async () => {
    setConfirm(null);
    try {
      await invoke<boolean>('api_delete_person', { personId });
      await onChanged();
      onSelect(null);
    } catch (error) {
      toast.error('Could not remove this person', { description: String(error) });
    }
  };

  if (loadError) {
    return <p className="p-6 text-sm text-red-600">{loadError}</p>;
  }
  if (!detail) {
    return <p className="p-6 text-sm text-gray-500">Loading…</p>;
  }

  const { person, meetings } = detail;
  const others = allPeople.filter((p) => p.id !== personId);

  return (
    <div className="max-w-3xl mx-auto p-6 space-y-6">
      {/* Name and actions */}
      <div className="flex items-start gap-3">
        <div className="flex-1 min-w-0">
          {isRenaming ? (
            <input
              autoFocus
              value={nameDraft}
              maxLength={120}
              onChange={(e) => setNameDraft(e.target.value)}
              onBlur={() => void saveName()}
              onKeyDown={(e) => {
                if (e.key === 'Enter') void saveName();
                if (e.key === 'Escape') setIsRenaming(false);
              }}
              aria-label="Person name"
              className="w-full text-2xl font-semibold text-gray-800 border-b-2 border-blue-500 bg-transparent focus:outline-none"
            />
          ) : (
            <h2 className="text-2xl font-semibold text-gray-800 truncate">{person.name}</h2>
          )}
          <p className="text-sm text-gray-500 mt-1">
            {meetings.length === 0
              ? 'Not in any meeting yet'
              : `${plural(meetings.length, 'meeting')} · ${plural(meetings.reduce((n, m) => n + m.line_count, 0), 'line')} spoken`}
          </p>
        </div>
        <div className="flex items-center gap-1 flex-shrink-0">
          <button
            onClick={() => {
              setNameDraft(person.name);
              setIsRenaming(true);
            }}
            className="p-2 rounded-md text-gray-500 hover:text-blue-600 hover:bg-blue-50"
            aria-label="Rename person"
            title="Rename"
          >
            <Pencil className="w-4 h-4" />
          </button>
          <Popover open={mergeOpen} onOpenChange={setMergeOpen}>
            <PopoverTrigger asChild>
              <button
                disabled={others.length === 0}
                className="p-2 rounded-md text-gray-500 hover:text-blue-600 hover:bg-blue-50 disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-gray-500"
                aria-label="Merge into another person"
                title={others.length === 0 ? 'Nobody else to merge with' : 'Merge into another person'}
              >
                <Merge className="w-4 h-4" />
              </button>
            </PopoverTrigger>
            <PopoverContent align="end" className="w-64 p-0">
              <Command>
                <CommandInput placeholder="Merge into…" />
                <CommandList>
                  <CommandEmpty>No match</CommandEmpty>
                  <CommandGroup heading={`Credit ${person.name}'s meetings to`}>
                    {others.map((p) => (
                      <CommandItem
                        key={p.id}
                        value={p.name}
                        onSelect={() => {
                          setMergeOpen(false);
                          setConfirm({ kind: 'merge', target: p });
                        }}
                      >
                        <span className="truncate">{p.name}</span>
                      </CommandItem>
                    ))}
                  </CommandGroup>
                </CommandList>
              </Command>
            </PopoverContent>
          </Popover>
          <button
            onClick={() => setConfirm({ kind: 'delete' })}
            className="p-2 rounded-md text-gray-500 hover:text-red-600 hover:bg-red-50"
            aria-label="Remove person"
            title="Remove from people"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Notes */}
      <section className="bg-white border border-gray-200 rounded-lg p-4">
        <div className="flex items-center justify-between mb-2">
          <h3 className="text-sm font-semibold text-gray-700">Notes</h3>
          <span className="text-xs text-gray-400 h-4">
            {notesState === 'saving' && 'Saving…'}
            {notesState === 'saved' && (
              <span className="inline-flex items-center text-green-600">
                <Check className="w-3 h-3 mr-1" />
                Saved
              </span>
            )}
          </span>
        </div>
        <textarea
          value={notes}
          onChange={(e) => {
            setNotes(e.target.value);
            setNotesState('idle');
          }}
          onBlur={() => void saveNotes()}
          placeholder="Role, organization, how to reach them, anything worth remembering…"
          aria-label="Notes"
          className="w-full min-h-[90px] px-3 py-2 text-sm border border-gray-200 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 resize-y"
        />
      </section>

      {/* Meetings */}
      <section className="bg-white border border-gray-200 rounded-lg">
        <h3 className="text-sm font-semibold text-gray-700 px-4 pt-4 pb-2">Meetings</h3>
        {meetings.length === 0 ? (
          <p className="px-4 pb-4 text-sm text-gray-500">
            Open a meeting and click a speaker label to link {person.name} to it.
          </p>
        ) : (
          <ul>
            {meetings.map((m) => (
              <li key={m.meeting_id}>
                <button
                  onClick={() => openMeeting(m.meeting_id, m.title)}
                  className="w-full text-left px-4 py-2.5 hover:bg-gray-50 border-t border-gray-100 flex items-baseline gap-3"
                >
                  <span className="flex-1 min-w-0 truncate text-sm font-medium text-gray-800">{m.title}</span>
                  <span className="text-xs text-gray-500 flex-shrink-0">
                    {formatDate(m.created_at)} · {plural(m.line_count, 'line')}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* What they said */}
      <section className="bg-white border border-gray-200 rounded-lg p-4">
        <h3 className="text-sm font-semibold text-gray-700 mb-2">What {person.name} said</h3>
        <div className="relative mb-3">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-gray-400" />
          <input
            value={lineQuery}
            onChange={(e) => setLineQuery(e.target.value)}
            placeholder="Search their lines across all meetings…"
            aria-label="Search what they said"
            className="w-full pl-8 pr-8 py-2 text-sm border border-gray-200 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
          {lineQuery && (
            <button
              onClick={() => setLineQuery('')}
              className="absolute right-2 top-2 text-gray-400 hover:text-gray-600"
              aria-label="Clear search"
            >
              <X className="h-4 w-4" />
            </button>
          )}
        </div>
        {lines.length === 0 ? (
          <p className="text-sm text-gray-500">
            {debouncedLineQuery.trim() ? 'Nothing matches.' : 'No spoken lines yet.'}
          </p>
        ) : (
          <ul className="divide-y divide-gray-100">
            {lines.map((line) => (
              <li key={line.transcript_id}>
                <button
                  onClick={() => openMeeting(line.meeting_id, line.meeting_title)}
                  className="w-full text-left py-2.5 hover:bg-gray-50 rounded"
                >
                  <div className="text-xs text-gray-500 mb-0.5">
                    {line.meeting_title} · {formatDate(line.meeting_created_at)}
                    {line.audio_start_time != null && ` · [${formatClock(line.audio_start_time)}]`}
                  </div>
                  <div className="text-sm text-gray-800">
                    <Highlight text={line.text} query={debouncedLineQuery} />
                  </div>
                </button>
              </li>
            ))}
          </ul>
        )}
        {lines.length >= LINE_LIMIT && (
          <p className="text-xs text-gray-400 mt-2">Showing the first {LINE_LIMIT} matches. Narrow the search to see others.</p>
        )}
      </section>

      {confirm?.kind === 'merge' && (
        <ConfirmDialog
          title="Merge people"
          body={`Credit all of ${person.name}'s meetings and notes to ${confirm.target.name}, and remove ${person.name} from your people? This cannot be undone.`}
          confirmLabel="Merge"
          onConfirm={() => void doMerge(confirm.target)}
          onClose={() => setConfirm(null)}
        />
      )}
      {confirm?.kind === 'delete' && (
        <ConfirmDialog
          title="Remove person"
          body={`Remove ${person.name} from your people? Their speaker labels go back to "Speaker N"; no transcripts or meetings are deleted.`}
          confirmLabel="Remove"
          destructive
          onConfirm={() => void doDelete()}
          onClose={() => setConfirm(null)}
        />
      )}
    </div>
  );
}
