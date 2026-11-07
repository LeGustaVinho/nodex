<template>
  <section>
    <input v-model="draft" @keyup.enter="add" />
    <ul>
      <li v-for="todo in visible" :key="todo.id" @click="toggle(todo)">{{ todo.text }}</li>
    </ul>
    <p>{{ remaining }} left</p>
  </section>
</template>

<script>
// Options API: Vue calls these options; `this` is the component.
export default {
  name: "TodoList",
  props: { filter: String },
  data() {
    return { draft: "", todos: [] };
  },
  computed: {
    visible() {
      return this.todos.filter((t) => !this.filter || t.text.includes(this.filter));
    },
    remaining() {
      return this.todos.filter((t) => !t.done).length;
    },
  },
  watch: {
    todos(next) {
      localStorage.setItem("todos", JSON.stringify(next));
    },
  },
  methods: {
    add() {
      if (!this.draft) {
        return;
      }
      this.todos.push({ id: Date.now(), text: this.draft, done: false });
      this.draft = "";
      this.$emit("added", this.todos.length);
    },
    toggle(todo) {
      todo.done = !todo.done;
    },
  },
  mounted() {
    this.todos = JSON.parse(localStorage.getItem("todos") || "[]");
  },
};
</script>

<style scoped>
section { padding: 1rem; }
</style>
