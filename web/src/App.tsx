import { Navigate, Route, Routes } from 'react-router-dom'
import { AppLayout } from './layout/AppLayout'
import DashboardPage from './pages/DashboardPage'
import FilesPage from './pages/FilesPage'
import FunctionsPage from './pages/FunctionsPage'
import SettingsPage from './pages/SettingsPage'
import TasksPage from './pages/TasksPage'

export default function App() {
  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route index element={<DashboardPage />} />
        <Route path="files" element={<FilesPage />} />
        <Route path="functions" element={<FunctionsPage />} />
        <Route path="tasks" element={<TasksPage />} />
        <Route path="settings" element={<SettingsPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  )
}
