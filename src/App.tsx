import { BrowserRouter, Routes, Route, Link, useLocation } from 'react-router-dom'
import Dashboard from './pages/Dashboard'
import Reports from './pages/Reports'
import Settings from './pages/Settings'

function Nav() {
  const location = useLocation()
  const linkClass = (path: string) =>
    `px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
      location.pathname === path
        ? 'bg-slate-800 text-amber-400'
        : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/50'
    }`

  return (
    <nav className="bg-slate-900 border-b border-slate-800">
      <div className="max-w-5xl mx-auto px-8 flex gap-1">
        <Link to="/" className={linkClass('/')}>
          Conciliacion
        </Link>
        <Link to="/reportes" className={linkClass('/reportes')}>
          Reportes
        </Link>
        <Link to="/ajustes" className={linkClass('/ajustes')}>
          Ajustes
        </Link>
      </div>
    </nav>
  )
}

export default function App() {
  return (
    <BrowserRouter>
      <div className="min-h-screen bg-slate-900 text-slate-100">
        <Nav />
        <main className="p-8">
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/reportes" element={<Reports />} />
            <Route path="/ajustes" element={<Settings />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  )
}
