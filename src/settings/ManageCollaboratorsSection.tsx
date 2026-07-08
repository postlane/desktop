// SPDX-License-Identifier: BUSL-1.1
// checklist 24.4.14 — "Manage Collaborators", folded into Settings → Account.
// For each owned workspace, an expandable Collaborators row.

import { Accordion, Text } from '@mantine/core';
import { useProjectsContext } from '../context/ProjectsProvider';
import CollaboratorsPanel from './CollaboratorsPanel';

export default function ManageCollaboratorsSection() {
  const { projects } = useProjectsContext();
  const owned = projects.filter((project) => project.is_owner);

  if (owned.length === 0) return null;

  return (
    <div className="mb-4">
      <Text size="sm" fw={600} mb="xs">Workspaces</Text>
      <Accordion variant="separated">
        {owned.map((project) => (
          <Accordion.Item key={project.id} value={project.id}>
            <Accordion.Control>{project.name}</Accordion.Control>
            <Accordion.Panel>
              <Text size="xs" fw={600} mb="xs">Collaborators</Text>
              <CollaboratorsPanel projectId={project.id} />
            </Accordion.Panel>
          </Accordion.Item>
        ))}
      </Accordion>
    </div>
  );
}
