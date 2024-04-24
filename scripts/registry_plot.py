import seaborn as sns
import pandas as pd
import matplotlib.pyplot as plt

df = pd.read_csv("generated/registry-bench.csv")
df.replace(
    {"access_type": {"readers": "lecteurs", "writers": "écrivains"}}, inplace=True
)
df["access_type"] = df["access_type"].replace("readers", "lecteur")
time_df = df[["ratio", "access_type", "access_time"]].copy()
block_df = df[["ratio", "access_type", "block_ratios"]].copy()

# time plot

filename = "generated/registry_time_plot"
plt.clf()
ax = sns.barplot(data=time_df, x="ratio", y="access_time", hue="access_type", gap=0.05)
ax.set(xlabel="Nombre lecteurs / écrivains", ylabel="Temps (us)")
plt.legend(title="Type d'accès")
plt.savefig(filename, dpi=300)

# block plot

filename = "generated/registry_block_plot"
plt.clf()
ax = sns.barplot(
    data=block_df, x="ratio", y="block_ratios", hue="access_type", gap=0.05
)
ax.set(xlabel="Nombre lecteurs / écrivains", ylabel="Ratio de requêtes en attentes")
ax.set_ylim(0.9995, 1)

plt.legend(title="Type d'accès")
plt.savefig(filename, dpi=300)
