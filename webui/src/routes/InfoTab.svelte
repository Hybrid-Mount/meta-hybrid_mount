<script>
  import { onMount } from 'svelte';
  import { store } from '../lib/store.svelte';
  import { API } from '../lib/api';
  import { ICONS } from '../lib/constants';
  import './InfoTab.css';
  import Skeleton from '../components/Skeleton.svelte';

  const REPO_OWNER = 'YuzakiKokuban';
  const REPO_NAME = 'meta-hybrid_mount';
  const DONATE_LINK = `https://github.com/sponsors/${REPO_OWNER}`; 

  let contributors = $state([]);
  let loading = $state(true);
  let error = $state(false);

  onMount(async () => {
    await fetchContributors();
  });

  async function fetchContributors() {
    try {
      const res = await fetch(`https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/contributors`);
      if (!res.ok) throw new Error('Failed to fetch list');
      
      const basicList = await res.json();

      const filteredList = basicList.filter(user => {
        const isBotType = user.type === 'Bot';
        const hasBotName = user.login.toLowerCase().includes('bot');
        return !isBotType && !hasBotName;
      });

      const detailPromises = filteredList.map(async (user) => {
        try {
            const detailRes = await fetch(user.url);
            if (detailRes.ok) {
                const detail = await detailRes.json();
                return { ...user, bio: detail.bio, name: detail.name || user.login };
            }
        } catch (e) {
            console.warn('Failed to fetch detail for', user.login);
        }
        return user;
      });
      contributors = await Promise.all(detailPromises);
    } catch (e) {
      console.error(e);
      error = true;
    } finally {
      loading = false;
    }
  }

  function handleLink(e, url) {
    e.preventDefault();
    API.openLink(url);
  }
</script>

<div class="info-container">
  
  <div class="project-header">
    <div class="app-logo">
        <svg viewBox="0 0 24 24" width="32" height="32"><path d={ICONS.home} fill="currentColor"/></svg>
    </div>
    <span class="app-name">{store.L.common.appName}</span>
    <span class="app-version">{store.version}</span>
  </div>

  <div class="action-grid">
    <a href={`https://github.com/${REPO_OWNER}/${REPO_NAME}`} 
       class="action-card" 
       onclick={(e) => handleLink(e, `https://github.com/${REPO_OWNER}/${REPO_NAME}`)}>
        <svg viewBox="0 0 24 24" class="action-icon"><path d={ICONS.github} /></svg>
        <span class="action-label">{store.L.info.projectLink}</span>
    </a>
  
    <a href={DONATE_LINK} 
       class="action-card"
       onclick={(e) => handleLink(e, DONATE_LINK)}>
        <svg viewBox="0 0 24 24" class="action-icon donate-icon"><path d={ICONS.donate} /></svg>
        <span class="action-label">{store.L.info.donate}</span>
    </a>
  </div>

  <div>
    <div class="section-title">{store.L.info.contributors}</div>
    
    <div class="contributors-list">
        {#if loading}
            {#each Array(3) as _}
                <div class="contributor-bar">
                    <Skeleton width="48px" height="48px" borderRadius="50%" />
                    <div class="c-info">
                        <div class="skeleton-spacer">
                             <Skeleton width="120px" height="16px" />
                        </div>
                        <Skeleton width="200px" height="12px" />
                    </div>
                </div>
            {/each}
        {:else if error}
            <div class="error-message">
                {store.L.info.loadFail}
            </div>
        {:else}
            {#each contributors as user}
                <a href={user.html_url} 
                   class="contributor-bar"
                   onclick={(e) => handleLink(e, user.html_url)}>
                    <img src={user.avatar_url} alt={user.login} class="c-avatar" />
                    <div class="c-info">
                        <span class="c-name">{user.name || user.login}</span>
                        <span class="c-bio">
                            {user.bio || store.L.info.noBio}
                        </span>
                    </div>
                    <svg viewBox="0 0 24 24" class="c-link-icon"><path d={ICONS.share} /></svg>
                </a>
            {/each}
        {/if}
    </div>
  </div>

</div>