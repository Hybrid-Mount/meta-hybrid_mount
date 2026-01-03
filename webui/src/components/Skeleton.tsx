/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import './Skeleton.css';

interface Props {
  width?: string;
  height?: string;
  borderRadius?: string;
  style?: string;
  class?: string;
}

export default function Skeleton(props: Props) {
  const styles = {
    "--skeleton-width": props.width || '100%',
    "--skeleton-height": props.height || '20px',
    "--skeleton-radius": props.borderRadius || '12px',
  } as any;

  return (
    <div 
      class={`skeleton ${props.class || ''}`} 
      style={styles}
    ></div>
  );
}