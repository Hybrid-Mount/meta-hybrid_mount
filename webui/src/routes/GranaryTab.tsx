/**
 * Copyright 2025 Meta-Hybrid Mount Authors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { createSignal, createMemo, onMount, Show, For } from 'solid-js';
import { API } from '../lib/api';
import { store } from '../lib/store';
import type { Silo } from '../lib/types';
import Skeleton from '../components/Skeleton';
import BottomActions from '../components/BottomActions';
import './GranaryTab.css';
import { ICONS } from '../lib/constants';

import '@material/web/button/filled-tonal-button.js';
import '@material/web/button/filled-button.js';
import '@material/web/button/text-button.js';
import '@material/web/iconbutton/icon-button.js';
import '@material/web/iconbutton/filled-tonal-icon-button.js';
import '@material/web/icon/icon.js';
import '@material/web/dialog/dialog.js';
import '@material/web/textfield/outlined-text-field.js';

export default function GranaryTab() {
  const [silos, setSilos] = createSignal<Silo[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [busyId, setBusyId] = createSignal<string | null>(null);
  
  const [showRestoreDialog, setShowRestoreDialog] = createSignal(false);
  const [showDeleteDialog, setShowDeleteDialog] = createSignal(false);
  const [showCreateDialog, setShowCreateDialog] = createSignal(false);
  const [selectedSilo, setSelectedSilo] = createSignal<Silo | null>(null);
  const [newSiloReason, setNewSiloReason] = createSignal("");
  const [isCreating, setIsCreating] = createSignal(false);

  const I_RESTORE = "M13,3A9,9 0 0,0 4,12H1L4.89,15.89L4.96,16.03L9,12H6A7,7 0 0,1 13,5A7,7 0 0,1 20,12A7,7 0 0,1 13,19C11.07,19 9.32,18.2 8.06,16.94L6.64,18.36C8.27,20 10.5,21 13,21A9,9 0 0,0 22,12A9,9 0 0,0 13,3Z";
  const I_DELETE = "M19,4H15.5L14.5,3H9.5L8.5,4H5V6H19M6,19A2,2 0 0,0 8,21H16A2,2 0 0,0 18,19V7H6V19Z";
  const I_ADD = "M19,13H13V19H11V13H5V11H11V5H13V11H19V13Z";
  const I_AUTO = "M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2M12,4A8,8 0 0,1 20,12A8,8 0 0,1 12,20A8,8 0 0,1 4,12A8,8 0 0,1 12,4M12,6A6,6 0 0,0 6,12A6,6 0 0,0 12,18A6,6 0 0,0 18,12A6,6 0 0,0 12,6M12,8A4,4 0 0,1 16,12A4,4 0 0,1 12,16A4,4 0 0,1 8,12A4,4 0 0,1 12,8Z";
  const I_MANUAL = "M15,9H5V5H15M12,19A3,3 0 0,1 9,16A3,3 0 0,1 12,13A3,3 0 0,1 15,16A3,3 0 0,1 12,19M17,3H5C3.89,3 3,3.9 3,5V19A2,2 0 0,0 5,21H19A2,2 0 0,0 21,19V7L17,3Z";
  const I_EMPTY = "M19 5v14H5V5h14m0-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2z";

  const L_G = createMemo(() => store.L.granary || {});
  const L_C = createMemo(() => store.L.common || {});

  async function loadSilos() {
    setLoading(true);
    try {
      const list = await API.getGranaryList();
      setSilos(list);
    } catch (e) {
      store.showToast(L_G().loadError || "Failed to load Granary", "error");
    } finally {
      setLoading(false);
    }
  }

  function confirmRestore(silo: Silo) {
    setSelectedSilo(silo);
    setShowRestoreDialog(true);
  }

  function confirmDelete(silo: Silo) {
    setSelectedSilo(silo);
    setShowDeleteDialog(true);
  }

  async function performRestore() {
    const silo = selectedSilo();
    if (!silo) return;
    setShowRestoreDialog(false);
    setBusyId(silo.id);
    try {
      await API.restoreSilo(silo.id);
      store.showToast(L_G().restoreSuccess || "Restored successfully.", "success");
    } catch (e: any) {
      store.showToast(e.message, "error");
    } finally {
      setBusyId(null);
      setSelectedSilo(null);
    }
  }

  async function performDelete() {
    const silo = selectedSilo();
    if (!silo) return;
    setShowDeleteDialog(false);
    setBusyId(silo.id);
    try {
      await API.deleteSilo(silo.id);
      setSilos(silos().filter(s => s.id !== silo.id));
      store.showToast(L_G().deleteSuccess || "Deleted", "success");
    } catch (e: any) {
      store.showToast(e.message, "error");
    } finally {
      setBusyId(null);
      setSelectedSilo(null);
    }
  }

  function openCreateDialog() {
      setNewSiloReason("");
      setShowCreateDialog(true);
  }

  async function performCreate() {
      const reason = newSiloReason().trim() || L_G().manualBackup || "Manual Backup";
      setIsCreating(true);
      try {
          await API.createSilo(reason);
          setShowCreateDialog(false);
          store.showToast(L_G().createSuccess || "Snapshot created", "success");
          await loadSilos();
      } catch(e: any) {
          store.showToast(e.message, "error");
      } finally {
          setIsCreating(false);
      }
  }

  function formatTime(ts: number) {
    return new Date(ts * 1000).toLocaleString(undefined, {
        month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit'
    });
  }

  function isManual(reason: string) {
      const lower = reason.toLowerCase();
      return !lower.includes('auto') && !lower.includes('system') && !lower.includes('boot');
  }

  onMount(loadSilos);

  return (
    <>
      <md-dialog 
        open={showRestoreDialog()} 
        onclose={() => setShowRestoreDialog(false)} 
        class="transparent-scrim"
      >
        <div slot="headline">{L_G().dialogRestoreTitle || 'Restore?'}</div>
        <div slot="content">
          {(L_G().dialogRestoreContent || 'Restore {label}?').replace('{label}', selectedSilo()?.label || '')}
        </div>
        <div slot="actions">
          <md-text-button onClick={() => setShowRestoreDialog(false)}>
             {L_C().cancel || 'Cancel'}
          </md-text-button>
          <md-text-button onClick={performRestore}>
             {L_G().restore || 'Restore'}
          </md-text-button>
        </div>
      </md-dialog>

      <md-dialog 
        open={showDeleteDialog()} 
        onclose={() => setShowDeleteDialog(false)} 
        class="transparent-scrim"
      >
        <div slot="headline">{L_G().dialogDeleteTitle || 'Delete?'}</div>
        <div slot="content">
          {(L_G().dialogDeleteContent || 'Delete {label}?').replace('{label}', selectedSilo()?.label || '')}
        </div>
        <div slot="actions">
          <md-text-button onClick={() => setShowDeleteDialog(false)}>
              {L_C().cancel || 'Cancel'}
          </md-text-button>
          <md-text-button class="danger-btn" onClick={performDelete}>
              {L_G().delete || 'Delete'}
          </md-text-button>
        </div>
      </md-dialog>

      <md-dialog 
        open={showCreateDialog()} 
        onclose={() => !isCreating() && setShowCreateDialog(false)} 
        class="transparent-scrim"
      >
          <div slot="headline">{L_G().dialogCreateTitle || 'New Backup'}</div>
          <div slot="content" class="create-content">
              <md-outlined-text-field
                  label={L_G().dialogCreateLabel || "Note"}
                  value={newSiloReason()}
                  onInput={(e: any) => setNewSiloReason(e.currentTarget.value)}
                  class="full-width"
                  disabled={isCreating()}
              ></md-outlined-text-field>
          </div>
          <div slot="actions">
              <md-text-button 
                onClick={() => setShowCreateDialog(false)} 
                disabled={isCreating()}
              >
                  {L_C().cancel || 'Cancel'}
              </md-text-button>
              <md-text-button 
                onClick={performCreate} 
                disabled={isCreating()}
              >
                  {isCreating() ? (L_C().saving || 'Saving...') : (L_C().confirm || 'Save')}
              </md-text-button>
          </div>
      </md-dialog>

      <div class="granary-list">
        <Show when={!loading()} fallback={
          <For each={Array(3)}>{() =>
            <div class="silo-card">
               <div class="card-main">
                  <Skeleton width="48px" height="48px" borderRadius="16px" />
                  <div class="silo-skeleton-info">
                      <Skeleton width="60%" height="24px" />
                      <Skeleton width="40%" height="16px" />
                  </div>
               </div>
               <Skeleton width="100%" height="48px" borderRadius="12px" style="margin-top: 16px" />
            </div>
          }</For>
        }>
          <Show when={silos().length > 0} fallback={
            <div class="empty-state">
              <div class="empty-icon-wrapper">
                  <md-icon><svg viewBox="0 0 24 24"><path d={I_EMPTY} /></svg></md-icon>
              </div>
              <h3>{L_G().emptyTitle || 'Empty'}</h3>
              <p>{L_G().emptyDesc || 'No snapshots.'}</p>
            </div>
          }>
            <For each={silos()}>
              {(silo) => (
                <div class={`silo-card ${isManual(silo.reason) ? 'variant-manual' : 'variant-auto'}`}>
                  <div class="card-main">
                      <div class="type-icon-container">
                          <md-icon class="type-icon">
                              <svg viewBox="0 0 24 24"><path d={isManual(silo.reason) ? I_MANUAL : I_AUTO} /></svg>
                          </md-icon>
                      </div>

                      <div class="info-block">
                          <div class="silo-title">{silo.label}</div>
                          <div class="silo-meta-row">
                              <span class="reason-badge">{silo.reason}</span>
                              <span class="time-text">{formatTime(silo.timestamp)}</span>
                          </div>
                      </div>
                      
                      <div class="top-action">
                          <md-icon-button 
                              onClick={(e: Event) => { e.stopPropagation(); confirmDelete(silo); }}
                              disabled={busyId() !== null}
                              class="delete-btn"
                          >
                              <md-icon><svg viewBox="0 0 24 24"><path d={I_DELETE} /></svg></md-icon>
                          </md-icon-button>
                      </div>
                  </div>

                  <div class="card-actions">
                      <md-filled-tonal-button 
                          class="restore-btn"
                          onClick={(e: Event) => { e.stopPropagation(); confirmRestore(silo); }}
                          disabled={busyId() !== null}
                      >
                          <md-icon slot="icon"><svg viewBox="0 0 24 24"><path d={I_RESTORE} /></svg></md-icon>
                          {busyId() === silo.id ? (L_G().restoring || 'Restoring...') : (L_G().restoreAction || 'Restore')}
                      </md-filled-tonal-button>
                  </div>
                </div>
              )}
            </For>
          </Show>
        </Show>
      </div>

      <BottomActions>
          <md-filled-tonal-icon-button 
            onClick={loadSilos} 
            disabled={loading()}
            title={L_C().refresh || "Refresh"}
          >
            <md-icon><svg viewBox="0 0 24 24"><path d={ICONS.refresh} /></svg></md-icon>
          </md-filled-tonal-icon-button>
        
          <div class="spacer"></div>
         
          <md-filled-button 
            onClick={openCreateDialog} 
            disabled={isCreating()}
          >
            <md-icon slot="icon"><svg viewBox="0 0 24 24"><path d={I_ADD} /></svg></md-icon>
            {L_G().create || "Create"}
          </md-filled-button>
      </BottomActions>
    </>
  );
}