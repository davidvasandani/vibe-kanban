import { Badge } from '@vibe/ui/components/Badge';
import { Button } from '@vibe/ui/components/Button';
import type { Invitation } from 'shared/types';
import { MemberRole } from 'shared/types';
import { useTranslation } from 'react-i18next';
import { Check, Copy, Trash2 } from 'lucide-react';
import { useEffect, useState } from 'react';
import { getRemoteApiUrl } from '@/shared/lib/remoteApi';

interface PendingInvitationItemProps {
  invitation: Invitation;
  onRevoke?: (invitationId: string) => void;
  isRevoking?: boolean;
}

function buildInviteLink(token: string): string {
  const base = (getRemoteApiUrl() || window.location.origin).replace(/\/$/, '');
  return `${base}/invitations/${token}/accept`;
}

export function PendingInvitationItem({
  invitation,
  onRevoke,
  isRevoking,
}: PendingInvitationItemProps) {
  const { t } = useTranslation('organization');
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timeout = setTimeout(() => setCopied(false), 2000);
    return () => clearTimeout(timeout);
  }, [copied]);

  const handleRevoke = () => {
    const confirmed = window.confirm(
      `Are you sure you want to revoke the invitation for ${invitation.email}? This action cannot be undone.`
    );
    if (confirmed) {
      onRevoke?.(invitation.id);
    }
  };

  const handleCopyLink = async () => {
    const link = buildInviteLink(invitation.token);
    try {
      await navigator.clipboard.writeText(link);
      setCopied(true);
    } catch {
      // Clipboard API unavailable (e.g. insecure context) — fall back to prompt
      window.prompt(t('invitationList.copyLink'), link);
    }
  };

  return (
    <div className="flex items-center justify-between p-3 border rounded-lg">
      <div className="flex items-center gap-3">
        <div>
          <div className="font-medium text-sm">{invitation.email}</div>
          <div className="text-xs text-muted-foreground">
            {t('invitationList.invited', {
              date: new Date(invitation.created_at).toLocaleDateString(),
            })}
          </div>
        </div>
        <Badge
          variant={
            invitation.role === MemberRole.ADMIN ? 'default' : 'secondary'
          }
        >
          {t('roles.' + invitation.role.toLowerCase())}
        </Badge>
        <Badge variant="outline">{t('invitationList.pending')}</Badge>
      </div>
      <div className="flex items-center gap-1">
        <Button
          variant="ghost"
          size="sm"
          onClick={handleCopyLink}
          title={t('invitationList.copyLink')}
        >
          {copied ? (
            <Check className="h-4 w-4 mr-1.5" />
          ) : (
            <Copy className="h-4 w-4 mr-1.5" />
          )}
          {copied ? t('invitationList.copied') : t('invitationList.copyLink')}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          onClick={handleRevoke}
          disabled={isRevoking}
          title="Revoke invitation"
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}
