export type PageId = 'EditorPage' | 'settings' | 'ViewsPage';

export interface Page {
  id: PageId;
  name: string;
  icon?: string;
  component: (...args: any[]) => React.ReactNode;
  componentArgs: any[],
  details?: string;
  popoverContent?: React.ReactNode;
}