declare module 'react-force-graph-3d' {
  import * as React from 'react';

  // Upstream package does not ship TypeScript declarations.
  // We intentionally type it as `any` to avoid blocking builds.
  const ForceGraph3D: React.ComponentType<any>;
  export default ForceGraph3D;
}

