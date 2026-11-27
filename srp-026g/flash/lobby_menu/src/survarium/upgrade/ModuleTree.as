package survarium.upgrade
{
   import flash.utils.Dictionary;
   import survarium.constants.CosmeticSlot;
   import survarium.data.ModificationCarriers;
   import survarium.data.ModificationSlots;
   import survarium.data.ModificationWays;

   // A gun's modules as 0.69d0's workshop has them (docs/notes/modification-069.md): the slots where a
   // player puts something, the carriers (a rail adapter, a tactical handguard) that bring slots of
   // their own, and the slot each way of an optic stands in (the tables ModificationSlots,
   // ModificationCarriers, ModificationWays). What stands where follows from two things: what the
   // weapon kit holds (its scope on its way, its muzzle device, its grip: what a match gets), and the
   // carriers the player put on with nothing on them yet (`picks`: the lobby's alone). A carrier stands
   // in its slot while something stands in a slot it brings, else while it is picked there; a slot a
   // carrier brings is shown while that carrier stands.
   public class ModuleTree
   {

      // what a slot takes of the weapon kit (its table's list)
      public static const MUZZLE:String = "muzzle";

      public static const GRIP:String = "grip";

      public static const SCOPE:String = "scope";

      // nothing of the kit: carriers only
      public static const PLAIN:String = "plain";

      // the rows of markers a slot is in
      public static const ABOVE:String = "above";

      public static const BELOW:String = "below";

      // ModuleTrees by weapon dict id, made once
      protected static var _trees:Dictionary;

      public var weapon:int;

      // its slots along the gun, the muzzle first: {slot, name, row, own, order, takes}
      public var slots:Array = [];

      // its carriers: {slot (where it goes), module, name, brings (an Array of slots)}
      public var carriers:Array = [];

      // optic dict id: the slots of its ways, in the ways' order (the kit's way is the index)
      protected var _ways:Dictionary = new Dictionary();

      public function ModuleTree(weapon:int)
      {
         super();
         this.weapon = weapon;
      }

      // The tree of a weapon; null for one the tables have no slot of.
      public static function of(weaponDictId:int) : ModuleTree
      {
         if(!_trees)
         {
            _trees = made();
         }
         return _trees[weaponDictId] as ModuleTree;
      }

      protected static function made() : Dictionary
      {
         var trees:Dictionary = new Dictionary();
         var row:Object = null;
         var tree:ModuleTree = null;
         var lists:Array = [[MUZZLE,ModificationSlots.muzzle],[GRIP,ModificationSlots.grip],[SCOPE,ModificationSlots.scope],[PLAIN,ModificationSlots.plain]];
         for each(var list:Array in lists)
         {
            for each(row in list[1] as Array)
            {
               tree = trees[row.weapon] as ModuleTree;
               if(!tree)
               {
                  tree = new ModuleTree(int(row.weapon));
                  trees[row.weapon] = tree;
               }
               tree.slots.push({"slot":int(row.slot),"name":String(row.name),"row":String(row.row),"own":int(row.own) != 0,"order":int(row.order),"takes":list[0]});
            }
         }
         for each(tree in trees)
         {
            tree.slots.sortOn("order",Array.NUMERIC);
         }
         for each(row in ModificationCarriers.carriers)
         {
            tree = trees[row.weapon] as ModuleTree;
            if(tree)
            {
               tree.carriers.push({"slot":int(row.slot),"module":String(row.module),"name":String(row.name),"brings":String(row.brings).split(",").map(toInt)});
            }
         }
         for each(row in ModificationWays.ways)
         {
            tree = trees[row.weapon] as ModuleTree;
            if(tree)
            {
               if(!tree._ways[row.optic])
               {
                  tree._ways[row.optic] = [];
               }
               (tree._ways[row.optic] as Array)[int(row.way)] = int(row.slot);
            }
         }
         return trees;
      }

      protected static function toInt(cell:String, ... rest) : int
      {
         return int(cell);
      }

      // The weapon kit's slot (CosmeticSlot) of what a slot takes.
      public static function kitSlot(takes:String) : int
      {
         return takes == MUZZLE ? CosmeticSlot.MUZZLE : (takes == GRIP ? CosmeticSlot.GRIP : CosmeticSlot.CAMOUFLAGE);
      }

      public function slot(id:int) : Object
      {
         for each(var row:Object in this.slots)
         {
            if(row.slot == id)
            {
               return row;
            }
         }
         return null;
      }

      // The slot the kit's muzzle device or grip stands in: the one slot that takes it.
      public function slotTaking(takes:String) : Object
      {
         for each(var row:Object in this.slots)
         {
            if(row.takes == takes)
            {
               return row;
            }
         }
         return null;
      }

      // The slots an optic's ways stand in, in their order; empty for an optic of no way onto the gun.
      public function waysOf(optic:int) : Array
      {
         return this._ways[optic] is Array ? this._ways[optic] as Array : [];
      }

      // What stands in each slot of the kit's: {takes, item, way} by slot (an item the inventory's;
      // the scope where its way puts it, `way` it).
      public function standing(fitted:Fitted) : Dictionary
      {
         var standing:Dictionary = new Dictionary();
         var row:Object = null;
         var ways:Array = null;
         for each(var takes:String in [MUZZLE,GRIP])
         {
            row = this.slotTaking(takes);
            if(Boolean(row) && Boolean(fitted.item(takes)))
            {
               standing[row.slot] = {"takes":takes,"item":fitted.item(takes),"way":0};
            }
         }
         if(Boolean(fitted.scope))
         {
            ways = this.waysOf(int(fitted.scope.dictId));
            if(fitted.way < ways.length)
            {
               standing[ways[fitted.way]] = {"takes":SCOPE,"item":fitted.scope,"way":fitted.way};
            }
         }
         return standing;
      }

      // The carrier standing in `slot`: the one that brings a slot something stands in, else the one
      // picked there; null for none (the gun's own part, or nothing).
      public function carrierIn(slot:int, standing:Dictionary, picks:Object) : Object
      {
         var carrier:Object = null;
         var brought:int = 0;
         for each(carrier in this.carriers)
         {
            if(carrier.slot != slot)
            {
               continue;
            }
            for each(brought in carrier.brings)
            {
               if(Boolean(standing[brought]))
               {
                  return carrier;
               }
            }
         }
         for each(carrier in this.carriers)
         {
            if(carrier.slot == slot && Boolean(picks) && picks[slot] == carrier.module)
            {
               return carrier;
            }
         }
         return null;
      }

      // The carrier that brings `slot`, whether it stands or not; null for a slot of the gun's own.
      public function bringer(slot:int) : Object
      {
         for each(var carrier:Object in this.carriers)
         {
            if((carrier.brings as Array).indexOf(slot) != -1)
            {
               return carrier;
            }
         }
         return null;
      }

      // The slots shown, in their order: the gun's own and those of the carriers standing.
      public function shown(standing:Dictionary, picks:Object) : Array
      {
         var out:Array = [];
         var carrier:Object = null;
         for each(var row:Object in this.slots)
         {
            carrier = this.bringer(row.slot);
            if(!carrier || this.carrierIn(carrier.slot,standing,picks) == carrier)
            {
               out.push(row);
            }
         }
         return out;
      }
   }
}
