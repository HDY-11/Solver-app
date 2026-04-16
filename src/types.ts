export interface City {
  id: number;
  x: number;
  y: number;
}

export interface TspData {
  cities: City[];
  adjacency_list: Array<Array<[number, number]>>;
}

export type PageId = 'solver' | 'about' | 'settings' | 'EditorPage';

export interface Page {
  id: PageId;
  name: string;
  icon?: string;
  component: React.ReactNode;
  details?: string;
  popoverContent?: React.ReactNode;
}