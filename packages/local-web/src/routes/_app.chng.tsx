import { createFileRoute } from '@tanstack/react-router';
import { ChngPage } from '@/pages/chng/ChngPage';

export const Route = createFileRoute('/_app/chng')({
  component: ChngPage,
});
