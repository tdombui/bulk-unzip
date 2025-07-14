import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { FolderOpen, FileArchive, Music, Play, Square } from 'lucide-react'
import './App.css'

interface ZipFile {
  path: string
  size: number
}

interface Mp3File {
  path: string
  size: number
  has_metadata: boolean
}

interface UnzipOptions {
  directory: string
  output: string
  workers: number
  skip_existing: boolean
}

interface StripOptions {
  directory: string
  output?: string
  workers: number
  skip_clean: boolean
  keep_fields?: string
  remove_all: boolean
  dry_run: boolean
}

function App() {
  const [activeTab, setActiveTab] = useState<'unzip' | 'strip'>('unzip')
  const [zipFiles, setZipFiles] = useState<ZipFile[]>([])
  const [mp3Files, setMp3Files] = useState<Mp3File[]>([])
  const [isProcessing, setIsProcessing] = useState(false)
  const [results, setResults] = useState<string[]>([])
  
  // Unzip options
  const [unzipOptions, setUnzipOptions] = useState<UnzipOptions>({
    directory: '',
    output: 'extracted',
    workers: 4,
    skip_existing: false
  })
  
  // Strip options
  const [stripOptions, setStripOptions] = useState<StripOptions>({
    directory: '',
    workers: 4,
    skip_clean: false,
    remove_all: false,
    dry_run: false
  })

  const selectDirectory = () => {
    const selected = prompt('Enter directory path:')
    
    if (selected) {
      if (activeTab === 'unzip') {
        setUnzipOptions(prev => ({ ...prev, directory: selected }))
        scanZipFiles(selected)
      } else {
        setStripOptions(prev => ({ ...prev, directory: selected }))
        scanMp3Files(selected)
      }
    }
  }

  const selectOutputDirectory = () => {
    const selected = prompt('Enter output directory path:')
    
    if (selected) {
      if (activeTab === 'unzip') {
        setUnzipOptions(prev => ({ ...prev, output: selected }))
      } else {
        setStripOptions(prev => ({ ...prev, output: selected }))
      }
    }
  }

  const scanZipFiles = async (directory: string) => {
    try {
      const files = await invoke<ZipFile[]>('scan_zip_files', { directory })
      setZipFiles(files)
    } catch (error) {
      console.error('Error scanning zip files:', error)
    }
  }

  const scanMp3Files = async (directory: string) => {
    try {
      const files = await invoke<Mp3File[]>('scan_mp3_files', { directory })
      setMp3Files(files)
    } catch (error) {
      console.error('Error scanning MP3 files:', error)
    }
  }

  const handleUnzip = async () => {
    if (!unzipOptions.directory) return
    
    setIsProcessing(true)
    setResults([])
    
    try {
      const results = await invoke<string[]>('unzip_files', { options: unzipOptions })
      setResults(results)
    } catch (error) {
      setResults([`Error: ${error}`])
    } finally {
      setIsProcessing(false)
    }
  }

  const handleStrip = async () => {
    if (!stripOptions.directory) return
    
    setIsProcessing(true)
    setResults([])
    
    try {
      const results = await invoke<string[]>('strip_metadata', { options: stripOptions })
      setResults(results)
    } catch (error) {
      setResults([`Error: ${error}`])
    } finally {
      setIsProcessing(false)
    }
  }

  const formatFileSize = (bytes: number) => {
    const sizes = ['B', 'KB', 'MB', 'GB']
    if (bytes === 0) return '0 B'
    const i = Math.floor(Math.log(bytes) / Math.log(1024))
    return `${(bytes / Math.pow(1024, i)).toFixed(2)} ${sizes[i]}`
  }

  return (
    <div className="app">
      <header className="app-header">
        <h1>Bulk Unzip</h1>
        <p>Extract zip files and strip MP3 metadata with ease</p>
      </header>

      <div className="tabs">
        <button 
          className={`tab ${activeTab === 'unzip' ? 'active' : ''}`}
          onClick={() => setActiveTab('unzip')}
        >
          <FileArchive size={20} />
          Extract ZIP Files
        </button>
        <button 
          className={`tab ${activeTab === 'strip' ? 'active' : ''}`}
          onClick={() => setActiveTab('strip')}
        >
          <Music size={20} />
          Strip MP3 Metadata
        </button>
      </div>

      <div className="content">
        {activeTab === 'unzip' ? (
          <div className="unzip-section">
            <div className="section-header">
              <h2>Extract ZIP Files</h2>
              <button 
                className="select-button"
                onClick={selectDirectory}
                disabled={isProcessing}
              >
                <FolderOpen size={16} />
                Select Directory
              </button>
            </div>

            {unzipOptions.directory && (
              <div className="options">
                <div className="option-group">
                  <label>Input Directory:</label>
                  <span className="path">{unzipOptions.directory}</span>
                </div>
                
                <div className="option-group">
                  <label>Output Directory:</label>
                  <div className="path-input">
                    <input
                      type="text"
                      value={unzipOptions.output}
                      onChange={(e) => setUnzipOptions(prev => ({ ...prev, output: e.target.value }))}
                      disabled={isProcessing}
                    />
                    <button onClick={selectOutputDirectory} disabled={isProcessing}>
                      Browse
                    </button>
                  </div>
                </div>

                <div className="option-group">
                  <label>Workers:</label>
                  <input
                    type="number"
                    min="1"
                    max="16"
                    value={unzipOptions.workers}
                    onChange={(e) => setUnzipOptions(prev => ({ ...prev, workers: parseInt(e.target.value) || 1 }))}
                    disabled={isProcessing}
                  />
                </div>

                <div className="option-group">
                  <label>
                    <input
                      type="checkbox"
                      checked={unzipOptions.skip_existing}
                      onChange={(e) => setUnzipOptions(prev => ({ ...prev, skip_existing: e.target.checked }))}
                      disabled={isProcessing}
                    />
                    Skip existing directories
                  </label>
                </div>
              </div>
            )}

            {zipFiles.length > 0 && (
              <div className="file-list">
                <h3>Found {zipFiles.length} ZIP files:</h3>
                <div className="files">
                  {zipFiles.map((file, index) => (
                    <div key={index} className="file-item">
                      <span className="file-name">{file.path.split(/[/\\]/).pop()}</span>
                      <span className="file-size">{formatFileSize(file.size)}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {unzipOptions.directory && (
              <button 
                className="process-button"
                onClick={handleUnzip}
                disabled={isProcessing}
              >
                {isProcessing ? <Square size={16} /> : <Play size={16} />}
                {isProcessing ? 'Processing...' : 'Extract Files'}
              </button>
            )}
          </div>
        ) : (
          <div className="strip-section">
            <div className="section-header">
              <h2>Strip MP3 Metadata</h2>
              <button 
                className="select-button"
                onClick={selectDirectory}
                disabled={isProcessing}
              >
                <FolderOpen size={16} />
                Select Directory
              </button>
            </div>

            {stripOptions.directory && (
              <div className="options">
                <div className="option-group">
                  <label>Input Directory:</label>
                  <span className="path">{stripOptions.directory}</span>
                </div>
                
                <div className="option-group">
                  <label>Output Directory (optional):</label>
                  <div className="path-input">
                    <input
                      type="text"
                      value={stripOptions.output || ''}
                      onChange={(e) => setStripOptions(prev => ({ ...prev, output: e.target.value || undefined }))}
                      placeholder="Leave empty to modify in place"
                      disabled={isProcessing}
                    />
                    <button onClick={selectOutputDirectory} disabled={isProcessing}>
                      Browse
                    </button>
                  </div>
                </div>

                <div className="option-group">
                  <label>Workers:</label>
                  <input
                    type="number"
                    min="1"
                    max="16"
                    value={stripOptions.workers}
                    onChange={(e) => setStripOptions(prev => ({ ...prev, workers: parseInt(e.target.value) || 1 }))}
                    disabled={isProcessing}
                  />
                </div>

                <div className="option-group">
                  <label>
                    <input
                      type="checkbox"
                      checked={stripOptions.skip_clean}
                      onChange={(e) => setStripOptions(prev => ({ ...prev, skip_clean: e.target.checked }))}
                      disabled={isProcessing}
                    />
                    Skip files without metadata
                  </label>
                </div>

                <div className="option-group">
                  <label>
                    <input
                      type="checkbox"
                      checked={stripOptions.remove_all}
                      onChange={(e) => setStripOptions(prev => ({ ...prev, remove_all: e.target.checked }))}
                      disabled={isProcessing}
                    />
                    Remove all metadata
                  </label>
                </div>

                <div className="option-group">
                  <label>
                    <input
                      type="checkbox"
                      checked={stripOptions.dry_run}
                      onChange={(e) => setStripOptions(prev => ({ ...prev, dry_run: e.target.checked }))}
                      disabled={isProcessing}
                    />
                    Dry run (show what would be done)
                  </label>
                </div>

                {!stripOptions.remove_all && (
                  <div className="option-group">
                    <label>Keep fields (comma-separated):</label>
                    <input
                      type="text"
                      value={stripOptions.keep_fields || ''}
                      onChange={(e) => setStripOptions(prev => ({ ...prev, keep_fields: e.target.value || undefined }))}
                      placeholder="title,artist,album,year"
                      disabled={isProcessing}
                    />
                  </div>
                )}
              </div>
            )}

            {mp3Files.length > 0 && (
              <div className="file-list">
                <h3>Found {mp3Files.length} MP3 files:</h3>
                <div className="files">
                  {mp3Files.map((file, index) => (
                    <div key={index} className="file-item">
                      <span className="file-name">{file.path.split(/[/\\]/).pop()}</span>
                      <span className="file-size">{formatFileSize(file.size)}</span>
                      <span className={`metadata-status ${file.has_metadata ? 'has-metadata' : 'no-metadata'}`}>
                        {file.has_metadata ? 'Has metadata' : 'No metadata'}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {stripOptions.directory && (
              <button 
                className="process-button"
                onClick={handleStrip}
                disabled={isProcessing}
              >
                {isProcessing ? <Square size={16} /> : <Play size={16} />}
                {isProcessing ? 'Processing...' : 'Strip Metadata'}
              </button>
            )}
          </div>
        )}

        {results.length > 0 && (
          <div className="results">
            <h3>Results:</h3>
            <div className="results-list">
              {results.map((result, index) => (
                <div key={index} className="result-item">
                  {result}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

export default App 