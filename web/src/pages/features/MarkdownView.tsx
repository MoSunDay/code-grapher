import remarkGfm from 'remark-gfm'
import ReactMarkdown from 'react-markdown'
import type { ReactNode } from 'react'

/** GitHub-flavored markdown rendering for feature docs (dark UI). */
export function MarkdownView({ markdown }: { markdown: string }): ReactNode {
  return (
    <div className="markdown-view">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{markdown}</ReactMarkdown>
    </div>
  )
}
