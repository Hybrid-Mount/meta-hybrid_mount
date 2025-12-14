<script lang="ts">
  import { store } from '../lib/store.svelte';
  import { flip } from 'svelte/animate';
  import { fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import './Toast.css';
</script>
<div class="toast-container">
  {#each store.toasts as toast (toast.id)}
    <div
      class="toast toast-{toast.type}"
      animate:flip={{ duration: 300, easing: cubicOut }}
      in:fly={{ y: 20, duration: 300, easing: cubicOut }}
      out:fly={{ y: 10, duration: 200, opacity: 0 }}
      role="alert"
    >
      <span>{toast.text}</span>
    </div>
  {/each}
</div>