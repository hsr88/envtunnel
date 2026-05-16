import { useState, useEffect, useCallback, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { QRCodeCanvas } from 'qrcode.react'
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart'

interface PortStatus {
  port: number
  active: boolean
  url: string
  framework: string | null
}

interface Toast {
  id: number
  message: string
}

function App() {
  const [ip, setIp] = useState<string | null>(null)
  const [ports, setPorts] = useState<PortStatus[]>([])
  const [loading, setLoading] = useState(true)
  const [scanning, setScanning] = useState(false)
  const [selectedPort, setSelectedPort] = useState<PortStatus | null>(null)
  const [lastScan, setLastScan] = useState<Date | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [autostartEnabled, setAutostartEnabled] = useState(false)
  const [autostartLoading, setAutostartLoading] = useState(true)

  const [customPortInput, setCustomPortInput] = useState('')
  const [customPorts, setCustomPorts] = useState<number[]>([])
  const [customPath, setCustomPath] = useState('')

  const [toasts, setToasts] = useState<Toast[]>([])
  const toastIdRef = useRef(0)
  const prevPortsRef = useRef<PortStatus[]>([])
  const [freshPorts, setFreshPorts] = useState<Set<number>>(new Set())
  const qrWrapperRef = useRef<HTMLDivElement>(null)

  const addToast = (message: string) => {
    const id = ++toastIdRef.current
    setToasts(prev => [...prev, { id, message }])
    setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id))
    }, 4000)
  }

  const fetchIp = useCallback(async () => {
    try {
      const localIp = await invoke<string>('get_local_ip')
      setIp(localIp)
      setError(null)
      return localIp
    } catch (err) {
      setError('ERROR: Failed to retrieve local IP')
      setLoading(false)
      return null
    }
  }, [])

  const scanPorts = useCallback(async (targetIp: string) => {
    if (!targetIp) return
    setScanning(true)
    try {
      const results = await invoke<PortStatus[]>('scan_ports', { ip: targetIp, customPorts })
      setPorts(results)

      const prevActive = new Set(prevPortsRef.current.filter(p => p.active).map(p => p.port))
      const newFresh = new Set<number>()
      for (const port of results) {
        if (port.active && !prevActive.has(port.port)) {
          newFresh.add(port.port)
          addToast(`PORT ${port.port} ACTIVE${port.framework ? ` — ${port.framework}` : ''}`)
        }
      }
      if (newFresh.size > 0) {
        setFreshPorts(prev => new Set([...prev, ...newFresh]))
        setTimeout(() => {
          setFreshPorts(prev => {
            const next = new Set(prev)
            for (const p of newFresh) next.delete(p)
            return next
          })
        }, 5000)
      }
      prevPortsRef.current = results

      const activePorts = results.filter((p: PortStatus) => p.active)
      if (activePorts.length > 0) {
        if (!selectedPort || !results.find((p: PortStatus) => p.port === selectedPort.port && p.active)) {
          setSelectedPort(activePorts[0])
        }
      } else {
        setSelectedPort(null)
      }

      setLastScan(new Date())
      setError(null)
    } catch (err) {
      console.error('Scan error:', err)
    } finally {
      setScanning(false)
      setLoading(false)
    }
  }, [customPorts, selectedPort])

  useEffect(() => {
    let isMounted = true
    const init = async () => {
      const localIp = await fetchIp()
      if (localIp && isMounted) await scanPorts(localIp)
    }
    init()
    return () => { isMounted = false }
  }, [fetchIp, scanPorts])

  useEffect(() => {
    if (!ip) return
    const interval = setInterval(() => scanPorts(ip), 3000)
    return () => clearInterval(interval)
  }, [ip, scanPorts])

  useEffect(() => {
    const checkAutostart = async () => {
      try {
        setAutostartEnabled(await isEnabled())
      } catch (err) {
        console.error('Autostart check error:', err)
      } finally {
        setAutostartLoading(false)
      }
    }
    checkAutostart()
  }, [])

  const toggleAutostart = async () => {
    try {
      if (autostartEnabled) {
        await disable()
        setAutostartEnabled(false)
      } else {
        await enable()
        setAutostartEnabled(true)
      }
    } catch (err) {
      console.error('Autostart toggle error:', err)
    }
  }

  const handleAddCustomPort = () => {
    const port = parseInt(customPortInput, 10)
    if (!isNaN(port) && port > 0 && port < 65536 && !customPorts.includes(port)) {
      setCustomPorts(prev => [...prev, port])
      setCustomPortInput('')
    }
  }

  const handleRemoveCustomPort = (port: number) => {
    setCustomPorts(prev => prev.filter(p => p !== port))
  }

  const getQrUrl = () => {
    if (!selectedPort) return ''
    let url = selectedPort.url
    if (customPath) {
      const cleanPath = customPath.startsWith('/') ? customPath : '/' + customPath
      url = url.replace(/\/$/, '') + cleanPath
    }
    return url
  }

  const handleCopyUrl = async () => {
    const url = getQrUrl()
    if (url) {
      await navigator.clipboard.writeText(url)
      addToast('URL COPIED')
    }
  }

  const handleSaveQr = () => {
    const canvas = qrWrapperRef.current?.querySelector('canvas')
    if (!canvas) return
    const link = document.createElement('a')
    link.download = `envtunnel-${selectedPort?.port || 'qr'}.png`
    link.href = (canvas as HTMLCanvasElement).toDataURL('image/png')
    link.click()
  }

  const activeCount = ports.filter(p => p.active).length
  const activePortsList = ports.filter(p => p.active)

  return (
    <div className="h-full bg-obsidian flex flex-col font-mono select-none relative">
      {/* TOASTS */}
      <div className="fixed top-2 right-2 z-50 flex flex-col gap-1 pointer-events-none">
        {toasts.map(t => (
          <div key={t.id} className="bg-neon-green text-obsidian text-[10px] font-bold px-2 py-1 border border-neon-green animate-pulse">
            {t.message}
          </div>
        ))}
      </div>

      {/* HEADER */}
      <header className="border-b border-obsidian-border bg-obsidian-light shrink-0">
        <div className="flex items-center justify-between px-3 py-2">
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 bg-neon-green animate-pulse-glow" />
            <h1 className="text-sm font-bold tracking-widest text-text-primary">
              ENVTUNNEL
            </h1>
            <span className="text-[10px] text-text-muted border border-obsidian-border px-1">
              v1.0
            </span>
          </div>
          <div className="flex items-center gap-3">
            {scanning && (
              <span className="text-[10px] text-neon-yellow animate-pulse">
                [ SCANNING ]
              </span>
            )}
            <span className={`text-[10px] font-bold ${activeCount > 0 ? 'text-neon-green' : 'text-text-muted'}`}>
              {activeCount > 0 ? `ACTIVE: ${activeCount}` : 'NO ACTIVE PORTS'}
            </span>
          </div>
        </div>
      </header>

      {/* ERROR */}
      {error && (
        <div className="mx-3 mt-2 border border-neon-red bg-obsidian-light p-2 shrink-0">
          <span className="text-neon-red text-[10px] font-bold">{error}</span>
        </div>
      )}

      {/* MAIN */}
      <main className="flex-1 flex flex-col p-3 gap-2 overflow-hidden min-h-0">

        {/* IP + ACTIVE PORTS */}
        <section className="border border-obsidian-border bg-obsidian-light p-2 shrink-0">
          <div className="flex items-center gap-2 mb-1.5">
            <span className="text-[10px] font-bold text-text-secondary tracking-wider">
              IP: {ip || '---.---.---.---'}
            </span>
            {lastScan && (
              <span className="text-[10px] text-text-muted ml-auto">
                {lastScan.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', second: '2-digit' })}
              </span>
            )}
            <button
              onClick={() => ip && scanPorts(ip)}
              disabled={scanning || !ip}
              className="px-2 py-0.5 border border-text-muted bg-obsidian text-text-primary text-[10px] font-bold
                         hover:border-neon-green hover:text-neon-green
                         disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
              {scanning ? '...' : 'SCAN'}
            </button>
          </div>
          {activeCount > 0 ? (
            <div className="flex flex-wrap gap-1">
              {activePortsList.map((port) => (
                <button
                  key={port.port}
                  onClick={() => setSelectedPort(port)}
                  className={`border px-2 py-1 text-left transition-all relative
                    ${selectedPort?.port === port.port
                      ? 'border-neon-green bg-obsidian'
                      : 'border-obsidian-border bg-obsidian hover:border-neon-green'
                    }`}
                >
                  <div className="flex items-center gap-1.5">
                    <span className="text-[10px] font-bold text-text-primary">:{port.port}</span>
                    {freshPorts.has(port.port) && (
                      <div className="w-1.5 h-1.5 bg-neon-yellow animate-pulse" title="Fresh" />
                    )}
                    <div className="w-1.5 h-1.5 bg-neon-green animate-pulse-glow" />
                  </div>
                  {port.framework && (
                    <div className="text-[8px] text-neon-green mt-0.5">{port.framework}</div>
                  )}
                </button>
              ))}
            </div>
          ) : (
            <div className="text-[10px] text-text-muted py-1">
              NO ACTIVE PORTS — RUN DEV SERVER
            </div>
          )}
        </section>

        {/* QR CODE */}
        <section className="flex-1 border border-obsidian-border bg-obsidian-light p-2 flex flex-col min-h-0">
          <h2 className="text-[10px] font-bold text-text-secondary tracking-wider mb-1 shrink-0">
            &gt; QR_CODE
          </h2>

          {loading ? (
            <div className="flex-1 flex items-center justify-center">
              <span className="text-text-muted text-[10px] animate-pulse">[ INIT... ]</span>
            </div>
          ) : selectedPort ? (
            <div className="flex-1 flex flex-col items-center justify-center gap-2 min-h-0">
              <div ref={qrWrapperRef} className="border-2 border-neon-green bg-white p-1 shrink-0">
                <QRCodeCanvas
                  value={getQrUrl()}
                  size={260}
                  level="M"
                  includeMargin={false}
                />
              </div>
              <div className="text-center shrink-0">
                <div className="text-neon-green text-[10px] font-bold tracking-wider mb-1">
                  {getQrUrl()}
                </div>
                <div className="flex items-center justify-center gap-2">
                  <button
                    onClick={handleCopyUrl}
                    className="px-2 py-0.5 border border-text-muted text-[9px] font-bold text-text-primary
                               hover:border-neon-green hover:text-neon-green transition-colors"
                  >
                    COPY URL
                  </button>
                  <button
                    onClick={handleSaveQr}
                    className="px-2 py-0.5 border border-text-muted text-[9px] font-bold text-text-primary
                               hover:border-neon-green hover:text-neon-green transition-colors"
                  >
                    SAVE QR
                  </button>
                </div>
              </div>
            </div>
          ) : (
            <div className="flex-1 flex flex-col items-center justify-center gap-2 min-h-0">
              <div className="w-12 h-12 border border-text-muted flex items-center justify-center">
                <span className="text-text-muted text-lg">?</span>
              </div>
              <span className="text-text-muted text-[10px]">NO ACTIVE PORTS</span>
              <span className="text-text-muted text-[9px]">RUN DEV SERVER ON ANY PORT</span>
            </div>
          )}
        </section>

        {/* CUSTOM PORT + PATH */}
        <section className="border border-obsidian-border bg-obsidian-light p-2 shrink-0">
          <h2 className="text-[10px] font-bold text-text-secondary tracking-wider mb-1.5">
            &gt; CUSTOM
          </h2>
          <div className="flex gap-1.5 mb-1.5">
            <input
              type="number"
              placeholder="PORT"
              value={customPortInput}
              onChange={e => setCustomPortInput(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && handleAddCustomPort()}
              className="w-16 bg-obsidian border border-obsidian-border text-text-primary text-[10px] px-1.5 py-0.5 outline-none focus:border-neon-green"
            />
            <button
              onClick={handleAddCustomPort}
              className="px-2 py-0.5 border border-text-muted text-[9px] font-bold text-text-primary
                         hover:border-neon-green hover:text-neon-green transition-colors"
            >
              ADD
            </button>
            <input
              type="text"
              placeholder="/path?query"
              value={customPath}
              onChange={e => setCustomPath(e.target.value)}
              className="flex-1 bg-obsidian border border-obsidian-border text-text-primary text-[10px] px-1.5 py-0.5 outline-none focus:border-neon-green"
            />
          </div>
          {customPorts.length > 0 && (
            <div className="flex flex-wrap gap-1">
              {customPorts.map(p => (
                <span key={p} className="inline-flex items-center gap-1 border border-obsidian-border bg-obsidian px-1.5 py-0.5 text-[9px] text-text-secondary">
                  :{p}
                  <button onClick={() => handleRemoveCustomPort(p)} className="text-text-muted hover:text-neon-red">×</button>
                </span>
              ))}
            </div>
          )}
        </section>

        {/* SETTINGS */}
        <section className="border border-obsidian-border bg-obsidian-light p-2 shrink-0">
          <button
            onClick={toggleAutostart}
            disabled={autostartLoading}
            className="w-full flex items-center gap-2 hover:opacity-80 transition-opacity"
          >
            <div
              className={`w-4 h-4 border flex items-center justify-center shrink-0
                ${autostartEnabled ? 'border-neon-green bg-neon-green' : 'border-text-muted'}
              `}
            >
              {autostartEnabled && (
                <svg className="w-2.5 h-2.5 text-obsidian" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="4">
                  <polyline points="20 6 9 17 4 12" />
                </svg>
              )}
            </div>
            <span className="text-[10px] font-bold text-text-primary">
              START WITH WINDOWS
            </span>
            <span className="text-[9px] text-text-muted ml-auto">
              {autostartLoading ? '...' : autostartEnabled ? 'ON' : 'OFF'}
            </span>
          </button>
        </section>
      </main>

      {/* FOOTER */}
      <footer className="border-t border-obsidian-border bg-obsidian-light px-3 py-1 shrink-0">
        <div className="flex items-center justify-between text-[9px] text-text-muted">
          <span>LOCAL-FIRST / OFFLINE</span>
          <span>{ip || 'NO IP'}</span>
        </div>
      </footer>
    </div>
  )
}

export default App
