import { createFileRoute } from "@tanstack/react-router";
import { MkIdeasWorkspace } from "@/features/mkideas/ui/MkIdeasWorkspace";

export const Route = createFileRoute("/people")({
  component: () => <MkIdeasWorkspace area="people" />,
});
