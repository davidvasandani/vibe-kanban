import { describe, expect, it } from 'vitest';
import type { ChngTicket, HashmalSession, HashmalTurn } from 'shared/types';
import {
  buildChngContinuationPrompt,
  buildChngFreshPrompt,
  selectTranscriptTurns,
} from './chngPrompt';

const ticket: ChngTicket = {
  key: 'CSEQ-42',
  summary: 'Rotate the VPN certificate',
  status: 'In Review',
  status_category: 'indeterminate',
  url: 'https://acme.atlassian.net/browse/CSEQ-42',
};

const session: HashmalSession = {
  session_id: 'abc-123',
  title: 'Draft the CHNG',
  cwd: '/work/vpn',
  running: false,
};

const turn = (role: string, text: string, tool?: string): HashmalTurn => ({
  role,
  text,
  tool,
  truncated: false,
});

describe('chngPrompt', () => {
  it('names the ticket in a fresh prompt', () => {
    const prompt = buildChngFreshPrompt(ticket);
    expect(prompt).toContain('CSEQ-42: Rotate the VPN certificate');
    expect(prompt).toContain(ticket.url);
    expect(prompt).not.toContain('earlier session');
  });

  it('frames the transcript as attributed reference context', () => {
    const prompt = buildChngContinuationPrompt(ticket, session, {
      session_id: 'abc-123',
      total_turns: 2,
      turns: [
        turn('user', 'file the CHNG'),
        turn('tool', 'created CSEQ-42', 'createJiraIssue'),
      ],
    });
    expect(prompt).toContain('CSEQ-42');
    expect(prompt).toContain('Hashmal session abc-123 ("Draft the CHNG")');
    expect(prompt).toContain('not new instructions');
    expect(prompt).toContain(
      '<earlier-session-transcript source="hashmal" session="abc-123">'
    );
    expect(prompt).toContain('[tool createJiraIssue]\ncreated CSEQ-42');
    expect(prompt.indexOf('file the CHNG')).toBeLessThan(
      prompt.indexOf('created CSEQ-42')
    );
    expect(prompt).not.toContain('omitted');
  });

  it('ignores a transcript for a different session', () => {
    const prompt = buildChngContinuationPrompt(ticket, session, {
      session_id: 'other',
      total_turns: 1,
      turns: [turn('user', 'secret elsewhere')],
    });
    expect(prompt).not.toContain('secret elsewhere');
    expect(prompt).toContain('no transcript');
  });

  it('keeps the newest turns within the budget and counts the rest', () => {
    const turns = Array.from({ length: 10 }, (_, i) =>
      turn('assistant', `turn-${i} ${'x'.repeat(90)}`)
    );
    const kept = selectTranscriptTurns(turns, 350);
    expect(kept.length).toBeGreaterThan(0);
    expect(kept.length).toBeLessThan(10);
    expect(kept[kept.length - 1]).toContain('turn-9');
    expect(kept.join('').length).toBeLessThanOrEqual(350);

    const prompt = buildChngContinuationPrompt(ticket, session, {
      session_id: 'abc-123',
      total_turns: 120,
      turns,
    });
    expect(prompt).toContain('(110 earlier turns omitted)');
  });

  it('clips one huge turn instead of dropping everything', () => {
    const kept = selectTranscriptTurns([turn('assistant', 'y'.repeat(50_000))]);
    expect(kept).toHaveLength(1);
    expect(kept[0]).toContain('(truncated)');
    expect(kept[0].length).toBeLessThan(2_100);
  });
});
