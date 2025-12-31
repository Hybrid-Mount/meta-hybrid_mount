/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, For, Show } from 'solid-js';
import '@material/web/chips/chip-set.js';
import '@material/web/chips/input-chip.js';
import '@material/web/icon/icon.js';
import '@material/web/iconbutton/icon-button.js';
import './ChipInput.css';

interface Props {
  values: string[];
  placeholder?: string;
  onValuesChange: (newValues: string[]) => void;
}

export default function ChipInput(props: Props) {
  const [inputValue, setInputValue] = createSignal("");

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ',' || e.key === ' ') {
      e.preventDefault();
      addChip();
    } else if (e.key === 'Backspace' && inputValue() === '' && props.values.length > 0) {
      removeChip(props.values.length - 1);
    }
  }

  function addChip() {
    const val = inputValue().trim();
    if (val) {
      if (!props.values.includes(val)) {
        const newValues = [...props.values, val];
        props.onValuesChange(newValues);
      }
      setInputValue("");
    }
  }

  function removeChip(index: number) {
    const newValues = props.values.filter((_, i) => i !== index);
    props.onValuesChange(newValues);
  }

  return (
    <div class="chip-input-wrapper">
      <md-chip-set>
        <For each={props.values}>
          {(val, i) => (
            <md-input-chip 
              label={val} 
              remove-only 
              on:remove={() => removeChip(i())}
            ></md-input-chip>
          )}
        </For>
      </md-chip-set>
      
      <div class="input-row">
        <input 
          type="text" 
          class="chip-input-field" 
          value={inputValue()}
          onInput={(e) => setInputValue(e.currentTarget.value)}
          onKeyDown={handleKeyDown}
          onBlur={addChip}
          placeholder={props.placeholder || "Add item..."}
          enterkeyhint="done"
        />
        <Show when={inputValue().trim().length > 0}>
          <md-icon-button 
            onClick={addChip} 
            class="add-btn"
            role="button"
            tabIndex={0}
            title="Add tag"
          >
            <md-icon>
              <svg viewBox="0 0 24 24"><path d="M9 16.2L4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4L9 16.2z" /></svg>
            </md-icon>
          </md-icon-button>
        </Show>
      </div>
    </div>
  );
}