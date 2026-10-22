{{- define "logrix.name" -}}
{{- default .Chart.Name .Values.global.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "logrix.fullname" -}}
{{- if .Values.global.fullnameOverride }}
{{- .Values.global.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.global.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{- define "logrix.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "logrix.labels" -}}
helm.sh/chart: {{ include "logrix.chart" . }}
{{ include "logrix.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: logrix
{{- end }}

{{- define "logrix.selectorLabels" -}}
app.kubernetes.io/name: {{ include "logrix.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{- define "logrix.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "logrix.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{- define "logrix.ingesterServiceAccountName" -}}
{{- if and .Values.ingester.serviceAccount .Values.ingester.serviceAccount.name }}
{{- .Values.ingester.serviceAccount.name }}
{{- else }}
{{- include "logrix.serviceAccountName" . }}
{{- end }}
{{- end }}

{{- define "logrix.processorServiceAccountName" -}}
{{- if and .Values.processor.serviceAccount .Values.processor.serviceAccount.name }}
{{- .Values.processor.serviceAccount.name }}
{{- else }}
{{- include "logrix.serviceAccountName" . }}
{{- end }}
{{- end }}

{{- define "logrix.apiServiceAccountName" -}}
{{- if and .Values.api.serviceAccount .Values.api.serviceAccount.name }}
{{- .Values.api.serviceAccount.name }}
{{- else }}
{{- include "logrix.serviceAccountName" . }}
{{- end }}
{{- end }}

{{- define "logrix.webhookServiceAccountName" -}}
{{- if and .Values.webhook.serviceAccount .Values.webhook.serviceAccount.name }}
{{- .Values.webhook.serviceAccount.name }}
{{- else }}
{{- include "logrix.serviceAccountName" . }}
{{- end }}
{{- end }}

{{- define "logrix.internalSecretName" -}}
{{- printf "%s-internal" (include "logrix.fullname" .) | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "logrix.credentialsSecretName" -}}
{{- if .Values.config.existingSecret }}
{{- .Values.config.existingSecret }}
{{- else }}
{{- include "logrix.internalSecretName" . }}
{{- end }}
{{- end }}

{{- define "logrix.processorSqsTriggerAuthenticationName" -}}
{{- default (printf "%s-processor-aws" (include "logrix.fullname" .)) .Values.processor.keda.triggerAuthentication.name | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "logrix.webhookSqsTriggerAuthenticationName" -}}
{{- default (printf "%s-webhook-aws" (include "logrix.fullname" .)) .Values.webhook.autoscaling.triggerAuthentication.name | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "logrix.processorRabbitmqTriggerAuthenticationName" -}}
{{- default (printf "%s-processor-rabbitmq" (include "logrix.fullname" .)) .Values.processor.keda.rabbitmqTriggerAuthentication.name | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "logrix.webhookRabbitmqTriggerAuthenticationName" -}}
{{- default (printf "%s-webhook-rabbitmq" (include "logrix.fullname" .)) .Values.webhook.autoscaling.rabbitmqTriggerAuthentication.name | trunc 63 | trimSuffix "-" }}
{{- end }}
