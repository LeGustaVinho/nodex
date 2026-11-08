<script setup lang="ts">
import { ref } from "vue";
import { label } from "./format";

const count = ref(0);

function add() {
  count.value = count.value + 1;
}
</script>

<template>
  <main>
    <h1>{{ label(count) }}</h1>
    <button id="add" @click="add">Add</button>
  </main>
</template>
