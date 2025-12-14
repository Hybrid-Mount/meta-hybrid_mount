<script lang="ts">
  import '@material/web/chips/chip-set.js';
  import '@material/web/chips/input-chip.js';
  import '@material/web/icon/icon.js';
  import '@material/web/iconbutton/icon-button.js';

  interface Props {
    values: string[];
    placeholder?: string;
    onChange?: () => void;
  }

  let { values = $bindable([]), placeholder = "Add item...", onChange }: Props = $props();
  let inputValue = $state("");

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ',' || e.key === ' ') {
      e.preventDefault();
      addChip();
    } else if (e.key === 'Backspace' && inputValue === '' && values.length > 0) {
      removeChip(values.length - 1);
    }
  }

  function addChip() {
    const val = inputValue.trim();
    if (val) {
      if (!values.includes(val)) {
        values = [...values, val];
        if (onChange) onChange();
      }
      inputValue = "";
    }
  }

  function removeChip(index: number) {
    values = values.filter((_, i) => i !== index);
    if (onChange) onChange();
  }
</script>

<div class="chip-input-wrapper">
  <md-chip-set>
    {#each values as val, i}
      <md-input-chip 
        label={val} 
        remove-only 
        onremove={() => removeChip(i)}
      ></md-input-chip>
    {/each}
  </md-chip-set>
  
  <div class="input-row">
    <input 
      type="text" 
      class="chip-input-field" 
      bind:value={inputValue} 
      onkeydown={handleKeydown}
      onblur={addChip}
      {placeholder}
      enterkeyhint="done"
    />
    {#if inputValue.trim().length > 0}
      <md-icon-button 
        onclick={addChip} 
        class="add-btn"
        aria-label="Add item"
        role="button"
        tabindex="0"
        onkeydown={() => {}}
      >
        <md-icon>
          <svg viewBox="0 0 24 24"><path d="M9 16.2L4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4L9 16.2z" /></svg>
        </md-icon>
      </md-icon-button>
    {/if}
  </div>
</div>

<style>
  .chip-input-wrapper {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border: 1px solid var(--md-sys-color-outline);
    border-radius: 8px;
    transition: border-color 0.2s;
  }
  .chip-input-wrapper:focus-within {
    border-color: var(--md-sys-color-primary);
    border-width: 2px;
    padding: 11px;
  }
  .input-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .chip-input-field {
    flex: 1;
    min-width: 80px;
    border: none;
    background: transparent;
    font-size: 16px;
    color: var(--md-sys-color-on-surface);
    outline: none;
    height: 32px;
    font-family: var(--md-ref-typeface-plain);
  }
  .chip-input-field::placeholder {
    color: var(--md-sys-color-on-surface-variant);
    opacity: 0.7;
  }
  .add-btn {
    --md-icon-button-icon-color: var(--md-sys-color-primary);
    width: 32px;
    height: 32px;
  }
</style>