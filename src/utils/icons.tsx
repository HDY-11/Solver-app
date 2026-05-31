// utils/icons.tsx — FontAwesome 图标统一映射（文本名称）
// 所有组件用语义化名称引用图标，如 icon="folder"、icon="python"

import {
  faFolder, faFolderPlus, faFile, faFilePen, faRotate,
  faPenToSquare, faTag, faTrashCan, faChartBar, faSearch,
  faGlobe, faTable, faScroll, faBox,
  faNoteSticky, faImage, faFloppyDisk, faBolt,
  faMinus, faXmark, faWindowMaximize, faSquare,
  faTableList, faGear, faCircleQuestion,
  faChevronRight, faChevronDown, faChevronLeft,
  faHouse, faPlay, faPlus, faThumbtack, faDownload,
  faCircleCheck, faCircleXmark, faTriangleExclamation, faCircleInfo,
  faCircle, faCheck, faSpinner, faRotateLeft,
  faSun, faMoon, faSatelliteDish, faEye, faCode,
  type IconDefinition,
} from '@fortawesome/free-solid-svg-icons';
import { faPython } from '@fortawesome/free-brands-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';

/** 文本名称 → FA 图标映射 */
const iconMap: Record<string, IconDefinition> = {
  folder:         faFolder,
  'folder-plus':  faFolderPlus,
  file:           faFile,
  'file-pen':     faFilePen,
  rotate:         faRotate,
  edit:           faPenToSquare,
  tag:            faTag,
  trash:          faTrashCan,
  chart:          faChartBar,
  search:         faSearch,
  python:         faPython,
  globe:          faGlobe,
  table:          faTable,
  scroll:         faScroll,
  box:            faBox,
  note:           faNoteSticky,
  image:          faImage,
  save:           faFloppyDisk,
  bolt:           faBolt,
  pin:            faThumbtack,
  minus:          faMinus,
  square:         faSquare,
  maximize:       faWindowMaximize,
  xmark:          faXmark,
  menu:           faTableList,
  gear:           faGear,
  question:       faCircleQuestion,
  'chevron-right': faChevronRight,
  'chevron-down':  faChevronDown,
  'chevron-left':  faChevronLeft,
  home:           faHouse,
  play:           faPlay,
  plus:           faPlus,
  download:       faDownload,
  success:        faCircleCheck,
  error:          faCircleXmark,
  warning:        faTriangleExclamation,
  info:           faCircleInfo,
  circle:         faCircle,
  check:          faCheck,
  spinner:        faSpinner,
  restore:        faRotateLeft,
  sun:            faSun,
  moon:           faMoon,
  signal:         faSatelliteDish,
  eye:            faEye,
  code:           faCode,
};

/** 根据文本名称获取 FA 图标定义（未知回退为 faFile） */
function getIcon(name: string): IconDefinition {
  return iconMap[name] ?? faFile;
}

/** 图标渲染组件 */
export function Icon({ icon, className, style }: {
  icon: string;
  className?: string;
  style?: React.CSSProperties;
}) {
  return (
    <FontAwesomeIcon
      icon={getIcon(icon)}
      className={className}
      style={{ width: '1em', height: '1em', ...style }}
    />
  );
}

// ── 直接导出常用图标（类型安全）──
export {
  faFolder, faFolderPlus, faFile, faFilePen, faRotate,
  faPenToSquare, faTag, faTrashCan, faChartBar, faSearch,
  faGlobe, faTable, faScroll, faBox,
  faNoteSticky, faImage, faFloppyDisk, faBolt,
  faMinus, faXmark, faWindowMaximize, faSquare,
  faTableList, faGear, faCircleQuestion,
  faChevronRight, faChevronDown, faChevronLeft, faHouse,
  faPlay, faPlus, faThumbtack, faPython, faDownload,
};
export { FontAwesomeIcon };
