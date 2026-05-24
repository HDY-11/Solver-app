import { RendererDef, PanelDef } from './types';

const rendererRegistry = new Map<string, RendererDef>();
const panelRegistry = new Map<string, PanelDef>();

export function registerRenderer(def: RendererDef) {
  rendererRegistry.set(def.name, def);
}

export function registerPanel(def: PanelDef) {
  panelRegistry.set(def.name, def);
}

export function getRenderer(name: string): RendererDef | undefined {
  return rendererRegistry.get(name);
}

export function getRendererByExtension(ext: string): RendererDef | undefined {
  for (const def of rendererRegistry.values()) {
    if (def.extensions.includes(ext)) return def;
  }
  return undefined;
}

export function getPanel(name: string): PanelDef | undefined {
  return panelRegistry.get(name);
}

export function getAllRenderers(): RendererDef[] {
  return Array.from(rendererRegistry.values());
}