import { createFileRoute } from '@tanstack/react-router';
import { WorkspacesCarousel } from '@/pages/workspaces/WorkspacesCarousel';

export const Route = createFileRoute('/_app/workspaces_/carousel')({
  component: WorkspacesCarousel,
});
