import {
  useMemo,
  useCallback,
  useState,
  useRef,
  useEffect,
  type Ref,
} from 'react';
import { useDropzone } from 'react-dropzone';
import { useTranslation } from 'react-i18next';
import { IssueProvider } from '@/integrations/remote/IssueProvider';
import { useIssueContext } from '@/shared/hooks/useIssueContext';
import { useScratch } from '@/shared/hooks/useScratch';
import { useDebouncedCallback } from '@/shared/hooks/useDebouncedCallback';
import { useOrgContext } from '@/shared/hooks/useOrgContext';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import { useCurrentUser } from '@/shared/hooks/auth/useCurrentUser';
import { useAzureAttachments } from '@/shared/hooks/useAzureAttachments';
import {
  commitCommentAttachments,
  deleteAttachment,
} from '@/shared/lib/remoteApi';
import {
  extractAttachmentIds,
  removeAttachmentMarkdownBySource,
  replaceAttachmentSource,
} from '@/shared/lib/attachmentUtils';
import {
  IssueCommentsSection,
  type IssueCommentsEditorProps,
  type IssueCommentData,
  type ReactionGroup,
} from '@vibe/ui/components/IssueCommentsSection';
import {
  IssueCommentDeliveryChip,
  IssueCommentIntentToggle,
} from '@vibe/ui/components/IssueCommentDelivery';
import { useIssueCommentDelivery } from '@/shared/hooks/useIssueCommentDelivery';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import {
  candidateLabel,
  defaultCommentIntent,
  describeDelivery,
  type CommentIntent,
} from '@/shared/lib/issueCommentDelivery';
import WYSIWYGEditor, {
  type WYSIWYGEditorRef,
} from '@/shared/components/WYSIWYGEditor';
import { MemberRole } from 'shared/remote-types';
import { ScratchType } from 'shared/types';

interface IssueCommentsSectionContainerProps {
  issueId: string;
}

/**
 * Container that wraps IssueCommentsSection with IssueProvider.
 * Manages comment data transformation, mutations, and UI state.
 */
export function IssueCommentsSectionContainer({
  issueId,
}: IssueCommentsSectionContainerProps) {
  return (
    <IssueProvider issueId={issueId}>
      <IssueCommentsSectionContent />
    </IssueProvider>
  );
}

function IssueCommentsSectionContent() {
  const { t } = useTranslation('common');
  const { membersWithProfilesById } = useOrgContext();
  const { projectId } = useProjectContext();
  const issueContext = useIssueContext();
  const { data: currentUser } = useCurrentUser();
  const currentUserId = currentUser?.user_id ?? '';
  const appNavigation = useAppNavigation();
  const delivery = useIssueCommentDelivery(issueContext.issueId);

  // Note vs. reply to agent. The default follows the backend: reply only when
  // exactly one linked session is waiting or running. An explicit choice
  // holds until the comment is posted.
  const [intentOverride, setIntentOverride] = useState<CommentIntent | null>(
    null
  );
  const targets = delivery.targets;
  const replyUnavailable =
    !delivery.available ||
    !targets ||
    targets.unavailable_reason !== null ||
    targets.candidates.length === 0;
  const intent: CommentIntent = replyUnavailable
    ? 'note'
    : (intentOverride ?? defaultCommentIntent(targets));

  // Check if current user is admin
  const currentUserMember = currentUserId
    ? membersWithProfilesById.get(currentUserId)
    : undefined;
  const isCurrentUserAdmin = currentUserMember?.role === MemberRole.ADMIN;

  // Ref to comment editor for programmatic focus
  const commentEditorRef = useRef<WYSIWYGEditorRef>(null);

  // UI state for comment input
  const [commentInput, setCommentInput] = useState('');
  const commentDraftId = `issue-comment:${issueContext.issueId}`;
  const {
    scratch: commentDraftScratch,
    updateScratch: updateCommentDraft,
    deleteScratch: deleteCommentDraft,
    isLoading: isCommentDraftLoading,
  } = useScratch(ScratchType.DRAFT_TASK, commentDraftId);
  const commentDraft =
    commentDraftScratch?.payload?.type === 'DRAFT_TASK'
      ? commentDraftScratch.payload.data
      : undefined;
  const hydratedCommentDraftIdRef = useRef<string | null>(null);
  const skipNextPersistRef = useRef(false);

  const persistCommentDraft = useCallback(
    async (value: string) => {
      try {
        if (!value.trim()) {
          await deleteCommentDraft();
          return;
        }

        await updateCommentDraft({
          payload: {
            type: 'DRAFT_TASK',
            data: value,
          },
        });
      } catch (e) {
        console.error('[IssueCommentsSection] Failed to persist draft:', e);
      }
    },
    [updateCommentDraft, deleteCommentDraft]
  );

  const {
    debounced: debouncedPersistCommentDraft,
    cancel: cancelDebouncedPersistCommentDraft,
  } = useDebouncedCallback(persistCommentDraft, 500);

  useEffect(() => {
    cancelDebouncedPersistCommentDraft();
    hydratedCommentDraftIdRef.current = null;
    skipNextPersistRef.current = false;
    setCommentInput('');
  }, [commentDraftId, cancelDebouncedPersistCommentDraft]);

  useEffect(() => {
    if (isCommentDraftLoading) return;
    if (hydratedCommentDraftIdRef.current === commentDraftId) return;

    const nextCommentInput = commentDraft ?? '';
    const shouldSkipNextPersist = nextCommentInput !== commentInput;

    hydratedCommentDraftIdRef.current = commentDraftId;
    skipNextPersistRef.current = shouldSkipNextPersist;
    setCommentInput(nextCommentInput);
  }, [isCommentDraftLoading, commentDraft, commentDraftId, commentInput]);

  const handleCommentMarkdownInsert = useCallback((markdown: string) => {
    setCommentInput((prev) =>
      prev.trim() ? `${prev}\n\n${markdown}` : markdown
    );
  }, []);

  const handleCommentSourceReplace = useCallback(
    (previousSrc: string, nextSrc: string) => {
      let didReplace = false;
      setCommentInput((prev) => {
        const { content, replaced } = replaceAttachmentSource(
          prev,
          previousSrc,
          nextSrc
        );
        didReplace = replaced;
        return content;
      });
      return didReplace;
    },
    []
  );

  const handleCommentSourceRemove = useCallback((src: string) => {
    let didRemove = false;
    setCommentInput((prev) => {
      const { content, removed } = removeAttachmentMarkdownBySource(prev, src);
      didRemove = removed;
      return content;
    });
    return didRemove;
  }, []);

  const {
    uploadFiles,
    getAttachmentIds,
    clearAttachments,
    isUploading,
    hasPendingAttachments,
    uploadError,
    clearUploadError,
    localAttachments,
  } = useAzureAttachments({
    projectId,
    onMarkdownInsert: handleCommentMarkdownInsert,
    onAttachmentSourceReplace: handleCommentSourceReplace,
    onAttachmentSourceRemove: handleCommentSourceRemove,
  });

  useEffect(() => {
    if (hydratedCommentDraftIdRef.current !== commentDraftId) return;
    if (hasPendingAttachments) return;
    if (skipNextPersistRef.current) {
      skipNextPersistRef.current = false;
      return;
    }

    debouncedPersistCommentDraft(commentInput);
  }, [
    commentInput,
    commentDraftId,
    debouncedPersistCommentDraft,
    hasPendingAttachments,
  ]);

  const { getRootProps, getInputProps, isDragActive, open } = useDropzone({
    onDrop: (acceptedFiles) => {
      if (acceptedFiles.length > 0) uploadFiles(acceptedFiles);
    },
    multiple: true,
    noClick: true,
    noKeyboard: true,
  });

  const onPasteFiles = useCallback(
    (files: File[]) => {
      if (files.length > 0) uploadFiles(files);
    },
    [uploadFiles]
  );

  // UI state for editing
  const [editingCommentId, setEditingCommentId] = useState<string | null>(null);
  const [editingValue, setEditingValue] = useState('');

  // Transform IssueComment to IssueCommentData
  const commentsData = useMemo<IssueCommentData[]>(() => {
    return issueContext.comments
      .map((comment) => {
        const author = comment.author_id
          ? membersWithProfilesById.get(comment.author_id)
          : undefined;
        const isAuthor =
          comment.author_id !== null && comment.author_id === currentUserId;
        const canModify = isAuthor || isCurrentUserAdmin;
        return {
          id: comment.id,
          authorId: comment.author_id,
          authorName: comment.author_id
            ? author
              ? `${author.first_name ?? ''} ${author.last_name ?? ''}`.trim() ||
                author.email ||
                t('kanban.unknownUser')
              : t('kanban.unknownUser')
            : t('kanban.deletedUser'),
          message: comment.message,
          createdAt: comment.created_at,
          author: author ?? null,
          canModify,
        };
      })
      .sort(
        (a, b) =>
          new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
      );
  }, [
    issueContext.comments,
    membersWithProfilesById,
    currentUserId,
    isCurrentUserAdmin,
    t,
  ]);

  // Group reactions by comment, then by emoji
  const reactionsByCommentId = useMemo(() => {
    const result = new Map<string, ReactionGroup[]>();

    for (const comment of commentsData) {
      const commentReactions = issueContext.getReactionsForComment(comment.id);
      const emojiMap = new Map<
        string,
        {
          count: number;
          hasReacted: boolean;
          reactionId: string | undefined;
          userIds: string[];
        }
      >();

      for (const reaction of commentReactions) {
        const existing = emojiMap.get(reaction.emoji);
        const isCurrentUser = reaction.user_id === currentUserId;

        if (existing) {
          existing.count++;
          existing.userIds.push(reaction.user_id);
          if (isCurrentUser) {
            existing.hasReacted = true;
            existing.reactionId = reaction.id;
          }
        } else {
          emojiMap.set(reaction.emoji, {
            count: 1,
            hasReacted: isCurrentUser,
            reactionId: isCurrentUser ? reaction.id : undefined,
            userIds: [reaction.user_id],
          });
        }
      }

      const groups: ReactionGroup[] = Array.from(emojiMap.entries()).map(
        ([emoji, data]) => ({
          emoji,
          count: data.count,
          hasReacted: data.hasReacted,
          reactionId: data.reactionId,
          userNames: data.userIds.map((userId) => {
            const member = membersWithProfilesById.get(userId);
            return member
              ? `${member.first_name ?? ''} ${member.last_name ?? ''}`.trim() ||
                  member.email ||
                  t('kanban.unknownUser')
              : t('kanban.unknownUser');
          }),
        })
      );

      result.set(comment.id, groups);
    }

    return result;
  }, [commentsData, issueContext, currentUserId, membersWithProfilesById, t]);

  const handleSubmitComment = useCallback(async () => {
    if (!commentInput.trim()) return;
    const message = commentInput.trim();
    const sendToAgent = intent === 'reply';
    const { persisted } = issueContext.insertComment({
      issue_id: issueContext.issueId,
      message,
      parent_id: null,
    });
    setIntentOverride(null);
    if (sendToAgent) {
      // Deliver only once the comment is saved; the backend re-reads it from
      // the issue, so the agent gets exactly what the issue shows.
      persisted
        .then((saved) => delivery.requestDelivery(saved.id))
        .catch((err) =>
          console.error('[IssueCommentsSection] Comment not saved:', err)
        );
    }
    cancelDebouncedPersistCommentDraft();
    setCommentInput('');
    try {
      await deleteCommentDraft();
    } catch (e) {
      console.error('[IssueCommentsSection] Failed to clear draft:', e);
    }

    const allUploadedIds = getAttachmentIds();
    if (allUploadedIds.length > 0) {
      const referencedIds = extractAttachmentIds(message);
      const idsToCommit = allUploadedIds.filter((id) => referencedIds.has(id));
      const idsToDelete = allUploadedIds.filter((id) => !referencedIds.has(id));

      if (idsToCommit.length > 0) {
        try {
          const confirmedComment = await persisted;
          await commitCommentAttachments(confirmedComment.id, {
            attachment_ids: idsToCommit,
          });
        } catch (err) {
          console.error('Failed to commit comment attachments:', err);
        }
      }
      for (const id of idsToDelete) {
        deleteAttachment(id).catch((err) =>
          console.error('Failed to delete abandoned attachment:', err)
        );
      }
    }
    clearAttachments();
  }, [
    commentInput,
    intent,
    delivery,
    issueContext,
    getAttachmentIds,
    clearAttachments,
    cancelDebouncedPersistCommentDraft,
    deleteCommentDraft,
  ]);

  const handleStartEdit = useCallback(
    (commentId: string) => {
      const comment = commentsData.find((c) => c.id === commentId);
      if (comment) {
        setEditingCommentId(commentId);
        setEditingValue(comment.message);
      }
    },
    [commentsData]
  );

  const handleSaveEdit = useCallback(() => {
    if (!editingCommentId || !editingValue.trim()) return;
    issueContext.updateComment(editingCommentId, {
      message: editingValue.trim(),
    });
    setEditingCommentId(null);
    setEditingValue('');
  }, [editingCommentId, editingValue, issueContext]);

  const handleCancelEdit = useCallback(() => {
    setEditingCommentId(null);
    setEditingValue('');
  }, []);

  const handleDeleteComment = useCallback(
    (id: string) => {
      issueContext.removeComment(id);
    },
    [issueContext]
  );

  const handleToggleReaction = useCallback(
    (commentId: string, emoji: string) => {
      // Check if user already has this reaction
      const reactions = issueContext.getReactionsForComment(commentId);
      const existingReaction = reactions.find(
        (r) => r.user_id === currentUserId && r.emoji === emoji
      );

      if (existingReaction) {
        // Remove the reaction
        issueContext.removeReaction(existingReaction.id);
      } else {
        // Add the reaction
        issueContext.insertReaction({
          comment_id: commentId,
          emoji,
        });
      }
    },
    [issueContext, currentUserId]
  );

  const handleReply = useCallback(
    (authorName: string, message: string) => {
      // Get first line of the message for the quote
      const firstLine = message.split('\n')[0].trim();
      const truncatedLine =
        firstLine.length > 100 ? `${firstLine.slice(0, 100)}...` : firstLine;
      const quote = `> ${authorName} ${t('kanban.replyQuotePrefix')}\n> ${truncatedLine}`;
      setCommentInput(quote);
      // Focus editor after setting value (setTimeout ensures value is set first)
      setTimeout(() => {
        commentEditorRef.current?.focus();
      }, 0);
    },
    [t]
  );

  const renderEditor = useCallback(
    ({
      value,
      onChange,
      placeholder,
      className,
      disabled,
      autoFocus,
      onCmdEnter,
      onPasteFiles,
      localAttachments,
      editorRef,
    }: IssueCommentsEditorProps) => (
      <WYSIWYGEditor
        ref={editorRef as Ref<WYSIWYGEditorRef>}
        value={value}
        onChange={onChange}
        placeholder={placeholder}
        className={className}
        disabled={disabled}
        autoFocus={autoFocus}
        onCmdEnter={onCmdEnter}
        onPasteFiles={onPasteFiles}
        localAttachments={localAttachments}
      />
    ),
    []
  );

  const renderComposerControls = useCallback(() => {
    if (!delivery.available) {
      return (
        <span className="text-xs text-low">
          {t('kanban.delivery.unavailable')}
        </span>
      );
    }
    const defaultCandidate = targets?.candidates.find(
      (candidate) => candidate.session_id === targets.default_session_id
    );
    let hint: string;
    if (replyUnavailable) {
      hint = t(
        `kanban.delivery.reasons.${targets?.unavailable_reason ?? 'no_sessions'}`
      );
    } else if (intent === 'note') {
      hint = t('kanban.delivery.noteHint');
    } else if (defaultCandidate) {
      hint = t(
        defaultCandidate.agent_state === 'running'
          ? 'kanban.delivery.replyHintRunning'
          : 'kanban.delivery.replyHintWaiting',
        { session: candidateLabel(defaultCandidate) }
      );
    } else {
      hint = t('kanban.delivery.replyHintChoose');
    }
    return (
      <IssueCommentIntentToggle
        value={intent}
        onChange={setIntentOverride}
        noteLabel={t('kanban.delivery.noteOption')}
        replyLabel={t('kanban.delivery.replyOption')}
        replyDisabled={replyUnavailable}
        replyDisabledReason={replyUnavailable ? hint : undefined}
        hint={hint}
      />
    );
  }, [delivery.available, targets, replyUnavailable, intent, t]);

  const renderCommentFooter = useCallback(
    (commentId: string) => {
      if (!delivery.available) return null;
      const stored = delivery.deliveriesByComment.get(commentId);
      const comment = issueContext.comments.find((c) => c.id === commentId);
      const editedAfterDelivery = Boolean(
        stored?.state === 'delivered' &&
          stored.delivered_comment_updated_at &&
          comment &&
          Date.parse(comment.updated_at) >
            Date.parse(stored.delivered_comment_updated_at)
      );
      const model = describeDelivery({
        delivery: stored,
        editedAfterDelivery,
        requestError: delivery.requestErrors.get(commentId) ?? null,
        targets,
      });
      if (!model) return null;
      const reason = model.params.reason
        ? t(`kanban.delivery.reasons.${model.params.reason}`)
        : '';
      const workspaceId = model.workspaceId;
      return (
        <IssueCommentDeliveryChip
          tone={model.tone}
          label={t(`kanban.delivery.${model.labelKey}`, {
            ...model.params,
            reason,
          })}
          detail={model.detail}
          notice={
            model.editedAfterDelivery
              ? t('kanban.delivery.editedAfterDelivery')
              : null
          }
          actions={model.actions
            .filter((action) => action !== 'choose_target')
            .map((action) => ({
              key: action,
              label: t(
                action === 'send'
                  ? 'kanban.delivery.sendToAgent'
                  : 'kanban.delivery.retry'
              ),
              onClick: () => void delivery.requestDelivery(commentId),
            }))}
          targetChoice={
            model.actions.includes('choose_target') && targets
              ? {
                  label: t('kanban.delivery.chooseSession'),
                  options: targets.candidates.map((candidate) => ({
                    id: candidate.session_id,
                    label: candidateLabel(candidate),
                  })),
                  onSelect: (sessionId) =>
                    void delivery.requestDelivery(commentId, sessionId),
                }
              : null
          }
          link={
            model.labelKey === 'delivered' && workspaceId
              ? {
                  label: t('kanban.delivery.openConversation'),
                  onClick: () => appNavigation.goToWorkspace(workspaceId),
                }
              : null
          }
        />
      );
    },
    [delivery, issueContext.comments, targets, t, appNavigation]
  );

  return (
    <IssueCommentsSection
      renderComposerControls={renderComposerControls}
      renderCommentFooter={renderCommentFooter}
      comments={commentsData}
      commentInput={commentInput}
      onCommentInputChange={setCommentInput}
      onSubmitComment={handleSubmitComment}
      editingCommentId={editingCommentId}
      editingValue={editingValue}
      onEditingValueChange={setEditingValue}
      onStartEdit={handleStartEdit}
      onSaveEdit={handleSaveEdit}
      onCancelEdit={handleCancelEdit}
      onDeleteComment={handleDeleteComment}
      reactionsByCommentId={reactionsByCommentId}
      onToggleReaction={handleToggleReaction}
      onReply={handleReply}
      isLoading={issueContext.isLoading}
      commentEditorRef={commentEditorRef}
      onPasteFiles={onPasteFiles}
      localAttachments={localAttachments}
      dropzoneProps={{ getRootProps, getInputProps, isDragActive }}
      onBrowseAttachment={open}
      isUploading={isUploading}
      attachmentError={uploadError}
      onDismissAttachmentError={clearUploadError}
      renderEditor={renderEditor}
    />
  );
}
