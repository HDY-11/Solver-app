import { ReactNode, ComponentType } from 'react';

export interface RendererProps {
  nodeId: string | null;
}

export interface RendererDef {
  name: string;
  extensions: string[];
  component: ComponentType<RendererProps>;
  icon: string;
  label: string;
  toolbar?: (props: RendererProps) => ReactNode;
}

export interface PanelDef {
  name: string;
  component: ComponentType;
  label: string;
}