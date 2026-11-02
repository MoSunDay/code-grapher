import type { FileContent, FileList, Id } from './types'
import { apiClient } from './client'

export async function listFiles(repoId: Id): Promise<FileList> {
  return (await apiClient.get<FileList>(`/repos/${repoId}/files`)).data
}

export async function getFileContent(repoId: Id, path: string): Promise<FileContent> {
  return (await apiClient.get<FileContent>(`/repos/${repoId}/file`, { params: { path } })).data
}
