<script lang="ts">
  import './Skeleton.css';
  let { width = '100%', height = '20px', borderRadius = '12px', style = '' } = $props();
</script>
<div 
  class="skeleton" 
  style:width={width} 
  style:height={height} 
  style:border-radius={borderRadius}
  style="{style}"
></div>