<!--

    Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.

-->
<script setup lang="ts">
import { ref } from "vue";
import {
  MiuixCard,
  MiuixArrowPreference,
  MiuixBottomSheet,
  MiuixSmallTitle,
  MiuixBasicComponent,
  MiuixText,
} from "miuix-vue";
import hybrid from "../compoments/logo.vue";
import { useI18n } from "vue-i18n";
import axios from "axios";
import { API } from "../lib/api";

interface Contributor {
  id: number;
  login: string;
  type: string;
  url: string;
  html_url: string;
  name: string;
  bio: string;
}

const version = ref("");

API.getVersion().then(function (response) {
  version.value = response;
});

const { t } = useI18n();
const open_donate = ref(false);
const open_koukuban_wechat = ref(false);
const open_koukuban_alipay = ref(false);
const open_koukuban_binance = ref(false);
const open_tools_cx_app_wechat = ref(false);
const contributors = ref([] as Contributor[]);

axios
  .get(
    "https://api.github.com/repos/hybrid-mount/meta-hybrid_mount/contributors",
  )
  .then(function (response) {
    console.info(response.data);
    contributors.value = response.data.filter(function (contributor) {
      return contributor.type === "User";
    });
    for (let i = 0; i < contributors.value.length; i++) {
      axios
        .get(contributors.value[i].url)
        .then(function (response) {
          if (response.data.bio === null) {
            contributors.value[i].bio = t("info.noBio");
          } else {
            contributors.value[i].bio = response.data.bio;
          }
          if (response.data.name === null) {
            contributors.value[i].name = contributors.value[i].login;
          } else {
            contributors.value[i].name = response.data.name;
          }
        })
        .catch(function (error) {
          console.error(error);
          contributors.value[i].bio = t("info.loadFail");
          contributors.value[i].name = contributors.value[i].login;
        });
    }
  })
  .catch(function (error) {
    console.error(error);
    contributors.value = [];
  });

console.info(contributors.value);
function run_hybird_api_command_open_url(url: string) {
  API.openLink(url);
}

function open_github_repo() {
  run_hybird_api_command_open_url(
    "https://github.com/hybrid-mount/meta-hybrid_mount",
  );
}

function open_telegram() {
  run_hybird_api_command_open_url("https://t.me/hybridmountchat");
}

function open_koukuban_paypal() {
  run_hybird_api_command_open_url("https://www.paypal.com/paypalme/LangQin280");
}
</script>

<template>
  <div class="page">
    <div class="hero">
      <div align="center">
        <hybrid />
      </div>
      <h1>{{ t("common.appName") }}</h1>
      <MiuixText>{{ version }}</MiuixText>
    </div>

    <MiuixCard class="ex-card">
      <MiuixArrowPreference
        :title="t('info.projectLink')"
        summary="github.com/hybrid-mount/meta-hybrid_mount"
        @click="open_github_repo"
      />
      <MiuixArrowPreference
        :title="t('info.telegram')"
        summary="@hybridmountchat"
        @click="open_telegram"
      />
      <MiuixArrowPreference
        :title="t('info.donate')"
        @click="open_donate = true"
      />
    </MiuixCard>

    <MiuixBottomSheet v-model="open_donate" :title="t('info.supportUs')">
      <MiuixSmallTitle :text="'YuzakiKokuban'" />
      <MiuixCard class="ex-mb12">
        <MiuixArrowPreference
          title="Alipay"
          @click="open_koukuban_alipay = true"
        />
        <MiuixArrowPreference
          title="Wechat"
          @click="open_koukuban_wechat = true"
        />
        <MiuixArrowPreference
          title="Binance"
          @click="open_koukuban_binance = true"
        />
        <MiuixArrowPreference title="PayPal" @click="open_koukuban_paypal" />
      </MiuixCard>
      <MiuixSmallTitle :text="'Tool-cx-app'" />
      <MiuixArrowPreference
        title="Wechat"
        @click="open_tools_cx_app_wechat = true"
      />
    </MiuixBottomSheet>

    <MiuixSmallTitle :text="t('info.contributors')" />
    <MiuixCard class="ex-card" :title="contributors.length">
      <div
        v-if="contributors.length > 0"
        v-for="contributor in contributors"
        :key="contributor.id"
      >
        <MiuixArrowPreference
          :title="contributor.name"
          :summary="contributor.bio"
          @click="run_hybird_api_command_open_url(contributor.html_url)"
        />
      </div>
      <div v-else>
        <MiuixBasicComponent :title="t('info.loadFail')" />
      </div>
    </MiuixCard>

    <div
      v-if="open_koukuban_alipay"
      style="
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.7);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 9999;
      "
      @click="open_koukuban_alipay = false"
    >
      <img
        src="/assets/donate/yuzaki_alipay.jpg"
        alt=""
        style="width: 400px; max-width: 80%; height: auto"
      />
    </div>
    <div
      v-if="open_koukuban_wechat"
      style="
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.7);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 9999;
      "
      @click="open_koukuban_wechat = false"
    >
      <img
        src="/assets/donate/yuzaki_wechat.jpg"
        alt=""
        style="width: 400px; max-width: 80%; height: auto"
      />
    </div>
    <div
      v-if="open_koukuban_binance"
      style="
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.7);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 9999;
      "
      @click="open_koukuban_binance = false"
    >
      <img
        src="/assets/donate/yuzaki_binance.jpg"
        alt=""
        style="width: 400px; max-width: 80%; height: auto"
      />
    </div>
    <div
      v-if="open_tools_cx_app_wechat"
      style="
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.7);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 9999;
      "
      @click="open_tools_cx_app_wechat = false"
    >
      <img
        src="/assets/donate/tools_wechat.jpg"
        alt=""
        style="width: 400px; max-width: 80%; height: auto"
      />
    </div>
  </div>
</template>

<style scoped>
.hero {
  position: relative;
  text-align: center;
  padding: 32px 0;

  .base {
    width: 170px;
    position: relative;
    z-index: 0;
  }

  h1 {
    font-size: 56px;
    letter-spacing: -1.68px;
    margin: 32px 0;
    font-weight: 500;
    color: var(--m-color-on-background);

    @media (max-width: 1024px) {
      font-size: 36px;
      margin: 20px 0;
    }
  }
}

.ex-card {
  margin: 0 12px 12px;
}

.ex-mb12 {
  margin-bottom: 12px;
}

.fade-scale-enter-active,
.fade-scale-leave-active {
  transition: all 0.3s ease;
}
.fade-scale-enter,
.fade-scale-leave-to {
  opacity: 0;
  transform: scale(0.8);
}
.fade-scale-enter-from,
.fade-scale-leave {
  opacity: 0;
  transform: scale(0.8);
}
</style>
