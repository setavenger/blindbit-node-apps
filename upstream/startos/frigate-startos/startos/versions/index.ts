import { VersionGraph } from '@start9labs/start-sdk'
import { current, FRIGATE_VERSION } from './current'

export { FRIGATE_VERSION }

export const versionGraph = VersionGraph.of({
  current,
  other: [],
})
