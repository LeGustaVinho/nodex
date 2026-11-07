<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";

// Composition API: the top level of <script setup> runs once per component instance.
const props = defineProps<{ items: string[] }>();
const emit = defineEmits<{ (e: "pick", item: string): void }>();
const query = ref("");
const matches = computed(() => props.items.filter((i) => i.toLowerCase().includes(query.value.toLowerCase())));

watch(query, (next) => {
  if (next.length > 20) {
    query.value = next.slice(0, 20);
  }
});

onMounted(() => {
  query.value = new URLSearchParams(location.search).get("q") ?? "";
});

function pick(item: string) {
  emit("pick", item);
}
</script>

<template>
  <input v-model="query" />
  <button v-for="m in matches" :key="m" @click="pick(m)">{{ m }}</button>
</template>
