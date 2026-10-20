import com.jpexs.decompiler.flash.abc.*;
import com.jpexs.decompiler.flash.abc.types.*;
import com.jpexs.decompiler.flash.abc.types.traits.*;
import java.util.*;
public class AnonymousNamespaceFixture {
    static int ns(ABC abc, int kind, String name) {
        var n = new Namespace(); n.kind = kind;
        n.name_index = name == null ? 0 : abc.constants.getStringId(name, true);
        return abc.constants.addNamespace(n);
    }
    static int qname(ABC abc, int namespace, String value) {
        var n = new Multiname(); n.kind = 7; n.namespace_index = namespace;
        n.name_index = abc.constants.getStringId(value, true);
        return abc.constants.addMultiname(n);
    }
    static void field(ABC abc, int namespace, String name) {
        var f = new TraitSlotConst(); f.name_index = qname(abc, namespace, name);
        abc.instance_info.get(0).instance_traits.traits.add(f);
    }
    static ABC base() {
        var abc = new ABC(null);
        ns(abc, 0x16, ""); ns(abc, 0x16, "fixture");
        ns(abc, 5, null); ns(abc, 5, null);
        var owner = new InstanceInfo(); owner.name_index = qname(abc, 2, "Probe");
        owner.instance_traits = new Traits(); abc.instance_info.add(owner);
        var c = new ClassInfo(); c.static_traits = new Traits(); abc.class_info.add(c);
        abc.constants.addNamespaceSet(new NamespaceSet(new int[]{1, 2, 3, 4}));
        return abc;
    }
    public static void main(String[] args) throws Exception {
        for (String mode : List.of("empty", "bound", "compiled-empty", "compiled-bound", "qualified-only", "qualified-changed", "qualified-ambiguous", "changed-binding", "qname", "protected", "namespace-value", "split-incidence", "ambiguous-anchor", "named-original", "no-public")) {
            var old = base(); var fresh = base();
            int first = ns(fresh, 5, "fixture:Probe"), second = ns(fresh, 5, "Probe.as$7");
            fresh.constants.addNamespaceSet(new NamespaceSet(new int[]{1, 2, first, second}));
            if (mode.startsWith("compiled-")) {
                old.constants.getNamespace(3).name_index = old.constants.getStringId("fixture:Probe", true);
                old.constants.getNamespace(4).name_index = old.constants.getStringId("Probe.as$1", true);
                fresh.constants.getNamespace(first).name_index = 0;
                fresh.constants.getNamespace(second).name_index = 0;
            }
            if (mode.equals("bound") || mode.equals("compiled-bound") || mode.equals("changed-binding")) {
                field(old, 3, "secret"); field(fresh, first, mode.endsWith("bound") ? "secret" : "changed");
            }
            if (mode.equals("qname")) qname(fresh, first, "unowned");
            if (mode.equals("protected")) fresh.instance_info.get(0).protectedNS = first;
            if (mode.equals("namespace-value")) {
                var value = new TraitSlotConst(); value.name_index = qname(fresh, 1, "value");
                value.value_kind = 5; value.value_index = first;
                fresh.instance_info.get(0).instance_traits.traits.add(value);
            }
            if (mode.equals("split-incidence")) old.constants.addNamespaceSet(new NamespaceSet(new int[]{1, 3}));
            if (mode.equals("ambiguous-anchor")) {
                int a = ns(old, 5, null), b = ns(old, 5, null);
                // Keep both constant pools' retained prefix aligned.
                var actual = base(); ns(actual, 5, null); ns(actual, 5, null);
                first = ns(actual, 5, "fixture:Probe"); second = ns(actual, 5, "Probe.as$7");
                actual.constants.addNamespaceSet(new NamespaceSet(new int[]{1, 2, first, second})); fresh = actual;
                old.constants.addNamespaceSet(new NamespaceSet(new int[]{1, 2, a, b}));
            }
            if (mode.equals("named-original")) old.constants.getNamespace(3).name_index = old.constants.getStringId("other", true);
            if (mode.equals("no-public")) {
                old.constants.getNamespaceSet(1).namespaces = new int[]{2, 3, 4};
                fresh.constants.getNamespaceSet(2).namespaces = new int[]{2, first, second};
            }
            if (mode.startsWith("qualified-")) {
                old.constants.getNamespaceSet(1).namespaces = new int[]{1, 2};
                fresh.constants.getNamespaceSet(1).namespaces = new int[]{1, 2};
                fresh.constants.getNamespaceSet(2).namespaces = new int[]{1, 2};
                field(old, 3, "secret");
                field(fresh, first, mode.equals("qualified-changed") ? "changed" : "secret");
                if (mode.equals("qualified-ambiguous")) field(old, 4, "secret");
            }
            var mapping = MovieSource.anonymousNamespaceLayout(old, fresh);
            boolean valid = mode.equals("empty") || mode.equals("bound") || mode.startsWith("compiled-");
            if (mode.equals("qualified-only")) {
                if (!Objects.equals(mapping.get(first), 3) || mapping.containsKey(second))
                    throw new AssertionError(mode + " " + mapping);
            } else if (valid) {
                if (!Objects.equals(mapping.get(first), 3) || !Objects.equals(mapping.get(second), 4))
                    throw new AssertionError(mode + " " + mapping);
                var full = MovieSource.namespaceLayout(old, fresh);
                if (!Objects.equals(full.get(first), 3) || !Objects.equals(full.get(second), 4))
                    throw new AssertionError("full " + mode + " " + full);
            } else if (mapping.containsKey(first)) throw new AssertionError("Accepted " + mode + " " + mapping);
        }
        System.exit(0);
    }
}
