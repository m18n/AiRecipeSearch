<script setup lang="ts">
import { ref, onMounted } from 'vue'
import {
  getAppliances,
  addAppliance,
  updateAppliance,
  deleteAppliance,
  getCookware,
  addCookware,
  updateCookware,
  deleteCookware,
} from '@/api/userCookingProfile'

interface Appliance {
  id: number
  name: string
  description: string | null
}

interface Cookware {
  id: number
  name: string
  description: string | null
}

interface EditState {
  name: string
  description: string
}


const appliances = ref<Appliance[]>([])
const appliancesLoading = ref(false)
const appliancesError = ref<string | null>(null)

const newAppliance = ref<EditState>({ name: '', description: '' })
const addingAppliance = ref(false)
const addApplianceError = ref<string | null>(null)

const editingApplianceId = ref<number | null>(null)
const editingAppliance = ref<EditState>({ name: '', description: '' })
const savingAppliance = ref(false)


const cookwareList = ref<Cookware[]>([])
const cookwareLoading = ref(false)
const cookwareError = ref<string | null>(null)

const newCookware = ref<EditState>({ name: '', description: '' })
const addingCookware = ref(false)
const addCookwareError = ref<string | null>(null)

const editingCookwareId = ref<number | null>(null)
const editingCookware = ref<EditState>({ name: '', description: '' })
const savingCookware = ref(false)


onMounted(async () => {
  await Promise.all([loadAppliances(), loadCookware()])
})


async function loadAppliances() {
  appliancesLoading.value = true
  appliancesError.value = null
  try {
    appliances.value = await getAppliances()
  } catch {
    appliancesError.value = 'Failed to load appliances.'
  } finally {
    appliancesLoading.value = false
  }
}

async function handleAddAppliance() {
  if (!newAppliance.value.name.trim()) return
  addingAppliance.value = true
  addApplianceError.value = null
  try {
    const created = await addAppliance({
      name: newAppliance.value.name.trim(),
      description: newAppliance.value.description.trim() || null,
    })
    appliances.value.push(created)
    newAppliance.value = { name: '', description: '' }
  } catch {
    addApplianceError.value = 'Failed to add appliance.'
  } finally {
    addingAppliance.value = false
  }
}

function startEditAppliance(appliance: Appliance) {
  editingApplianceId.value = appliance.id
  editingAppliance.value = {
    name: appliance.name,
    description: appliance.description ?? '',
  }
}

function cancelEditAppliance() {
  editingApplianceId.value = null
}

async function handleUpdateAppliance(id: number) {
  if (!editingAppliance.value.name.trim()) return
  savingAppliance.value = true
  try {
    const updated = await updateAppliance(id, {
      name: editingAppliance.value.name.trim(),
      description: editingAppliance.value.description.trim() || null,
    })
    const index = appliances.value.findIndex((a) => a.id === id)
    if (index !== -1) appliances.value[index] = updated
    editingApplianceId.value = null
  } catch {
    appliancesError.value = 'Failed to update appliance.'
  } finally {
    savingAppliance.value = false
  }
}

async function handleDeleteAppliance(id: number) {
  try {
    await deleteAppliance(id)
    appliances.value = appliances.value.filter((a) => a.id !== id)
  } catch {
    appliancesError.value = 'Failed to delete appliance.'
  }
}


async function loadCookware() {
  cookwareLoading.value = true
  cookwareError.value = null
  try {
    cookwareList.value = await getCookware()
  } catch {
    cookwareError.value = 'Failed to load cookware.'
  } finally {
    cookwareLoading.value = false
  }
}

async function handleAddCookware() {
  if (!newCookware.value.name.trim()) return
  addingCookware.value = true
  addCookwareError.value = null
  try {
    const created = await addCookware({
      name: newCookware.value.name.trim(),
      description: newCookware.value.description.trim() || null,
    })
    cookwareList.value.push(created)
    newCookware.value = { name: '', description: '' }
  } catch {
    addCookwareError.value = 'Failed to add cookware.'
  } finally {
    addingCookware.value = false
  }
}

function startEditCookware(item: Cookware) {
  editingCookwareId.value = item.id
  editingCookware.value = {
    name: item.name,
    description: item.description ?? '',
  }
}

function cancelEditCookware() {
  editingCookwareId.value = null
}

async function handleUpdateCookware(id: number) {
  if (!editingCookware.value.name.trim()) return
  savingCookware.value = true
  try {
    const updated = await updateCookware(id, {
      name: editingCookware.value.name.trim(),
      description: editingCookware.value.description.trim() || null,
    })
    const index = cookwareList.value.findIndex((c) => c.id === id)
    if (index !== -1) cookwareList.value[index] = updated
    editingCookwareId.value = null
  } catch {
    cookwareError.value = 'Failed to update cookware.'
  } finally {
    savingCookware.value = false
  }
}

async function handleDeleteCookware(id: number) {
  try {
    await deleteCookware(id)
    cookwareList.value = cookwareList.value.filter((c) => c.id !== id)
  } catch {
    cookwareError.value = 'Failed to delete cookware.'
  }
}
</script>

<template>
  <div class="kitchen-tools-view">
    <h1 class="page-title">My Kitchen Tools</h1>

    <section class="section">
      <h2 class="section-title">Kitchen Appliances</h2>

      <div v-if="appliancesLoading" class="status-message">Loading appliances…</div>
      <div v-if="appliancesError" class="error-message">{{ appliancesError }}</div>

     <form class="add-form" @submit.prevent="handleAddAppliance">
        <input
          v-model="newAppliance.name"
          class="input"
          type="text"
          placeholder="Appliance name (e.g. Blender)"
          required
        />
        <textarea
          v-model="newAppliance.description"
          class="input textarea"
          placeholder="Description (optional)"
          rows="3"
        />
        <button class="btn btn--primary" type="submit" :disabled="addingAppliance || !newAppliance.name.trim()">
          {{ addingAppliance ? 'Adding…' : 'Add Appliance' }}
        </button>
      </form>

      <div v-if="addApplianceError" class="error-message">{{ addApplianceError }}</div>

      <ul v-if="appliances.length" class="items-list">
        <li v-for="appliance in appliances" :key="appliance.id" class="item-row">

          <template v-if="editingApplianceId !== appliance.id">
            <div class="item-info">
              <span class="item-name">{{ appliance.name }}</span>
              <span v-if="appliance.description" class="item-description">{{ appliance.description }}</span>
            </div>
            <div class="item-actions">
              <button class="btn btn--secondary" @click="startEditAppliance(appliance)">Edit</button>
              <button class="btn btn--danger" @click="handleDeleteAppliance(appliance.id)">Delete</button>
            </div>
          </template>

          <template v-else>
            <div class="edit-fields">
              <input v-model="editingAppliance.name" class="input" type="text" placeholder="Name" required />
              <textarea v-model="editingAppliance.description" class="input textarea" placeholder="Description (optional)" rows="3" />
            </div>
            <div class="item-actions">
              <button
                class="btn btn--primary"
                :disabled="savingAppliance || !editingAppliance.name.trim()"
                @click="handleUpdateAppliance(appliance.id)"
              >
                {{ savingAppliance ? 'Saving…' : 'Save' }}
              </button>
              <button class="btn btn--secondary" @click="cancelEditAppliance">Cancel</button>
            </div>
          </template>

        </li>
      </ul>
      <p v-else-if="!appliancesLoading" class="empty-message">No appliances added yet.</p>
    </section>

    <section class="section">
      <h2 class="section-title">Cookware</h2>

      <div v-if="cookwareLoading" class="status-message">Loading cookware…</div>
      <div v-if="cookwareError" class="error-message">{{ cookwareError }}</div>

      <form class="add-form" @submit.prevent="handleAddCookware">
        <input
          v-model="newCookware.name"
          class="input"
          type="text"
          placeholder="Cookware name (e.g. Cast Iron Pan)"
          required
        />
        <textarea
          v-model="newCookware.description"
          class="input textarea"
          placeholder="Description (optional)"
          rows="3"
        />
        <button class="btn btn--primary" type="submit" :disabled="addingCookware || !newCookware.name.trim()">
          {{ addingCookware ? 'Adding…' : 'Add Cookware' }}
        </button>
      </form>
      <div v-if="addCookwareError" class="error-message">{{ addCookwareError }}</div>

      <ul v-if="cookwareList.length" class="items-list">
        <li v-for="item in cookwareList" :key="item.id" class="item-row">

          <template v-if="editingCookwareId !== item.id">
            <div class="item-info">
              <span class="item-name">{{ item.name }}</span>
              <span v-if="item.description" class="item-description">{{ item.description }}</span>
            </div>
            <div class="item-actions">
              <button class="btn btn--secondary" @click="startEditCookware(item)">Edit</button>
              <button class="btn btn--danger" @click="handleDeleteCookware(item.id)">Delete</button>
            </div>
          </template>

          <template v-else>
            <div class="edit-fields">
              <input v-model="editingCookware.name" class="input" type="text" placeholder="Name" required />
              <textarea v-model="editingCookware.description" class="input  textarea" placeholder="Description (optional)" rows="3" />
            </div>
            <div class="item-actions">
              <button
                class="btn btn--primary"
                :disabled="savingCookware || !editingCookware.name.trim()"
                @click="handleUpdateCookware(item.id)"
              >
                {{ savingCookware ? 'Saving…' : 'Save' }}
              </button>
              <button class="btn btn--secondary" @click="cancelEditCookware">Cancel</button>
            </div>
          </template>

        </li>
      </ul>
      <p v-else-if="!cookwareLoading" class="empty-message">No cookware added yet.</p>
    </section>
  </div>
</template>

<style scoped>
.textarea {
  resize: vertical;
  min-height: 4.5rem;
  line-height: 1.5;
}

.kitchen-tools-view {
  max-width: 760px;
  margin: 0 auto;
  padding: 2rem 1rem;
}

.page-title {
  font-size: 1.75rem;
  font-weight: 700;
  margin-bottom: 2rem;
}


.section {
  margin-bottom: 3rem;
}

.section-title {
  font-size: 1.25rem;
  font-weight: 600;
  margin-bottom: 1rem;
  padding-bottom: 0.5rem;
  border-bottom: 1px solid #e5e7eb;
}


.add-form {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
  margin-bottom: 1rem;
}


.input {
  flex: 1;
  min-width: 140px;
  padding: 0.5rem 0.75rem;
  border: 1px solid #d1d5db;
  border-radius: 0.375rem;
  font-size: 0.9rem;
  outline: none;
  transition: border-color 0.15s;
}

.input:focus {
  border-color: #6366f1;
  box-shadow: 0 0 0 2px rgba(99, 102, 241, 0.2);
}


.items-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.item-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.75rem 1rem;
  background: #f9fafb;
  border: 1px solid #e5e7eb;
  border-radius: 0.5rem;
  flex-wrap: wrap;
}

.item-info {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  flex: 1;
}

.item-name {
  font-weight: 500;
  font-size: 0.95rem;
}

.item-description {
  font-size: 0.82rem;
  color: #6b7280;
}

.edit-fields {
  display: flex;
  gap: 0.5rem;
  flex: 1;
  flex-wrap: wrap;
}

.item-actions {
  display: flex;
  gap: 0.5rem;
  flex-shrink: 0;
}


.btn {
  padding: 0.4rem 0.85rem;
  border: none;
  border-radius: 0.375rem;
  font-size: 0.875rem;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.15s, background-color 0.15s;
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn--primary {
  background-color: #6366f1;
  color: #fff;
}

.btn--primary:not(:disabled):hover {
  background-color: #4f46e5;
}

.btn--secondary {
  background-color: #e5e7eb;
  color: #374151;
}

.btn--secondary:not(:disabled):hover {
  background-color: #d1d5db;
}

.btn--danger {
  background-color: #fee2e2;
  color: #b91c1c;
}

.btn--danger:not(:disabled):hover {
  background-color: #fecaca;
}


.status-message {
  color: #6b7280;
  font-size: 0.9rem;
  margin-bottom: 0.75rem;
}

.empty-message {
  color: #9ca3af;
  font-size: 0.9rem;
  font-style: italic;
}

.error-message {
  color: #b91c1c;
  background: #fee2e2;
  border-radius: 0.375rem;
  padding: 0.5rem 0.75rem;
  font-size: 0.875rem;
  margin-bottom: 0.75rem;
}
@media (max-width: 640px) {
  .kitchen-tools-view {
    padding: 1.25rem 0.75rem;
  }

  .page-title {
    font-size: 1.4rem;
    margin-bottom: 1.25rem;
  }

  .add-form {
    flex-direction: column;
  }

  .input {
    min-width: 100%;
  }

  .btn {
    width: 100%;
    text-align: center;
  }

  .item-row {
    flex-direction: column;
    align-items: flex-start;
  }

  .item-actions {
    width: 100%;
    justify-content: flex-end;
  }

  .item-actions .btn {
    width: auto;
    flex: 1;
    text-align: center;
  }

  .edit-fields {
    width: 100%;
    flex-direction: column;
  }
}
</style>