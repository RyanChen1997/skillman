<script setup lang="ts">
import { computed } from "vue";
import { marked } from "marked";

const props = defineProps<{ source: string }>();
marked.setOptions({ breaks: true });
const html = computed(() => marked.parse(props.source) as string);
</script>

<template>
  <div class="w-full p-6 border border-[var(--color-border)] rounded-md bg-[var(--color-surface)] leading-7 text-sm" v-html="html"></div>
</template>
