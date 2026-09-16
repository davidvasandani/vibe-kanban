import type { DisplayEntry } from '@/shared/hooks/useConversationHistory/types';

const FULL_TIMESTAMP_OPTIONS: Intl.DateTimeFormatOptions = {
  dateStyle: 'medium',
  timeStyle: 'medium',
};

const SAME_DAY_OPTIONS: Intl.DateTimeFormatOptions = {
  hour: 'numeric',
  minute: '2-digit',
};

const OTHER_DAY_OPTIONS: Intl.DateTimeFormatOptions = {
  month: 'short',
  day: 'numeric',
  hour: 'numeric',
  minute: '2-digit',
};

export interface FormattedConversationTimestamp {
  dateTime: string;
  compactLabel: string;
  fullLabel: string;
}

function isSameLocalDay(left: Date, right: Date): boolean {
  return (
    left.getFullYear() === right.getFullYear() &&
    left.getMonth() === right.getMonth() &&
    left.getDate() === right.getDate()
  );
}

export function formatConversationTimestamp(
  timestamp: string | null | undefined,
  now = new Date(),
  locale?: string | string[]
): FormattedConversationTimestamp | null {
  if (!timestamp) return null;

  const date = new Date(timestamp);
  if (!Number.isFinite(date.getTime())) return null;

  const compactOptions = isSameLocalDay(date, now)
    ? SAME_DAY_OPTIONS
    : OTHER_DAY_OPTIONS;

  return {
    dateTime: timestamp,
    compactLabel: new Intl.DateTimeFormat(locale, compactOptions).format(date),
    fullLabel: new Intl.DateTimeFormat(locale, FULL_TIMESTAMP_OPTIONS).format(
      date
    ),
  };
}

function timestampForAtomicEntry(entry: DisplayEntry): string | null {
  if (entry.type !== 'NORMALIZED_ENTRY') return null;

  const entryType = entry.content.entry_type.type;
  if (
    entryType === 'loading' ||
    entryType === 'next_action' ||
    entryType === 'token_usage_info'
  ) {
    return null;
  }

  if (entryType === 'tool_use') {
    const action = entry.content.entry_type.action_type;
    if (
      action.action === 'ask_user_question' ||
      (action.action === 'plan_presentation' && !action.plan.trim())
    ) {
      return null;
    }
  }

  const eventTimestamp = entry.content.timestamp;
  if (eventTimestamp && Number.isFinite(new Date(eventTimestamp).getTime())) {
    return eventTimestamp;
  }

  return entry.processCreatedAt ?? null;
}

export function getDisplayEntryTimestamp(entry: DisplayEntry): string | null {
  if (
    entry.type !== 'AGGREGATED_GROUP' &&
    entry.type !== 'AGGREGATED_DIFF_GROUP' &&
    entry.type !== 'AGGREGATED_THINKING_GROUP'
  ) {
    return timestampForAtomicEntry(entry);
  }

  let latestTimestamp: string | null = null;
  let latestTime = Number.NEGATIVE_INFINITY;

  for (const member of entry.entries) {
    const timestamp = timestampForAtomicEntry(member);
    if (!timestamp) continue;

    const time = new Date(timestamp).getTime();
    if (Number.isFinite(time) && time > latestTime) {
      latestTime = time;
      latestTimestamp = timestamp;
    }
  }

  return latestTimestamp;
}

export function ConversationTimestamp({ entry }: { entry: DisplayEntry }) {
  const timestamp = formatConversationTimestamp(
    getDisplayEntryTimestamp(entry)
  );
  if (!timestamp) return null;

  return (
    <time
      dateTime={timestamp.dateTime}
      title={timestamp.fullLabel}
      aria-label={timestamp.fullLabel}
      className="mt-half block text-right text-sm text-low"
    >
      {timestamp.compactLabel}
    </time>
  );
}
