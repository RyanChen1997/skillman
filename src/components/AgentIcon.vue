<script setup lang="ts">
defineProps<{ agentId: string }>();
</script>

<template>
  <span class="agent-icon inline-block bg-current [mask-size:contain] [mask-repeat:no-repeat] [mask-position:center]" :class="agentId"></span>
</template>
