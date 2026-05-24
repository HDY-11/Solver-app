import { registerPanel } from '../registry/registry';

function SettingPanel() {
  return (
    <div style={{ padding: 24 }}>
      <h2>设置</h2>
      <p>偏好设置开发中...</p>
    </div>
  );
}

registerPanel({
  name: 'setting',
  component: SettingPanel,
  label: '设置',
});

export default SettingPanel;