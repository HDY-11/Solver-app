export type PageId = 'EditorPage' | 'settings';

export interface Page {
  id: PageId;
  name: string;
  icon?: string;
  component: React.ReactNode;
  details?: string;
  popoverContent?: React.ReactNode;
}