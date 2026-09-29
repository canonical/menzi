import { create } from 'zustand';
import { PROJECT_SECTIONS, isProjectSection, type ProjectSection } from '../lib/routes';

const PROJECT_KEY = 'menzi_active_project';
const SECTION_KEY = 'menzi_project_section';

function readStoredProject(): string {
  try {
    return localStorage.getItem(PROJECT_KEY) ?? '';
  } catch {
    return '';
  }
}

function readStoredSection(): ProjectSection {
  try {
    const stored = localStorage.getItem(SECTION_KEY) ?? '';
    return isProjectSection(stored) ? stored : PROJECT_SECTIONS[0];
  } catch {
    return PROJECT_SECTIONS[0];
  }
}

function writeStored(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    return;
  }
}

interface ActiveProjectState {
  projectId: string;
  section: ProjectSection;
  setProject: (projectId: string) => void;
  setSection: (section: ProjectSection) => void;
  recordRoute: (projectId: string, section: string) => void;
}

export const useActiveProject = create<ActiveProjectState>((set) => ({
  projectId: readStoredProject(),
  section: readStoredSection(),
  setProject: (projectId) => {
    writeStored(PROJECT_KEY, projectId);
    set({ projectId });
  },
  setSection: (section) => {
    writeStored(SECTION_KEY, section);
    set({ section });
  },
  recordRoute: (projectId, section) => {
    const nextSection = isProjectSection(section) ? section : PROJECT_SECTIONS[0];
    writeStored(PROJECT_KEY, projectId);
    writeStored(SECTION_KEY, nextSection);
    set({ projectId, section: nextSection });
  },
}));
