import { Alert } from 'antd'
import { useMemo, type ReactNode } from 'react'

const MAX_LINES = 4000

interface Props {
  content: string
  symbolLines?: Set<number>
}

export function SourceViewer({ content, symbolLines }: Props): ReactNode {
  const allLines = useMemo(() => content.split('\n'), [content])
  const truncated = allLines.length > MAX_LINES
  const lines = truncated ? allLines.slice(0, MAX_LINES) : allLines
  return (
    <div>
      {truncated && (
        <Alert
          type="warning"
          showIcon
          style={{ marginBottom: 8 }}
          message={`文件较大，仅显示前 ${MAX_LINES} 行（共 ${allLines.length} 行）`}
        />
      )}
      <div className="src-viewer">
        {lines.map((line, i) => (
          <div
            key={i}
            className={symbolLines?.has(i + 1) === true ? 'src-line is-symbol' : 'src-line'}
          >
            <span className="src-no">{i + 1}</span>
            <span className="src-text">{line}</span>
          </div>
        ))}
      </div>
    </div>
  )
}
