import { Routes, Route } from 'react-router-dom';
import ResourceView from './ResourceView.tsx';
import WelcomeView from './WelcomeView.tsx';

function Main() {
  return (
    <Routes>
      <Route path="/:container/:renderer/:content" element={<ResourceView />} />
      <Route path="/:container/:renderer" element={<ResourceView />} />
      <Route path="/:container" element={<WelcomeView />} />
      <Route path="/" element={<WelcomeView />} />
    </Routes>
  );
}

export default Main;