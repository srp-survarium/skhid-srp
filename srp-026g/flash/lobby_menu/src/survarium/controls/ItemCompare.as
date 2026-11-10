package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.text.TextField;
   import flash.utils.Dictionary;
   import survarium.Util;
   import survarium.core.Icon;
   import survarium.data.PropertiesDescriptor;
   import survarium.item.ItemFactory;
   import survarium.item.ModificationRolls;
   import survarium.item.AmmoDetails;
   import survarium.item.WeaponDetails;
   import survarium.player.LocalPlayerProfile;
   import survarium.player.MatchEffects;
   import survarium.constants.ModifierType;
   import survarium.constants.PropertyDirection;
   import survarium.constants.SuperCategory;
   import survarium.constants.ItemCategory;
   import survarium.constants.QuickSlotSize;
   import survarium.data.LocalizationData;
   
   public class ItemCompare extends MovieClip
   {
      
      protected static var _propsMin:Dictionary = new Dictionary();
      
      protected static var _propsMax:Dictionary = new Dictionary();
      
      protected static var _propsDir:Dictionary = new Dictionary();
      
      protected static var _propsDescriptor:PropertiesDescriptor;
      
      public var diff_1:MovieClip;
      
      public var diff_2:MovieClip;
      
      public var diff_3:MovieClip;
      
      public var diff_4:MovieClip;
      
      public var diff_5:MovieClip;
      
      public var diff_6:MovieClip;
      
      public var diff_7:MovieClip;
      
      public var diff_8:MovieClip;
      
      public var textFieldGun:TextField;
      
      protected var _primary:Object;
      
      protected var _secondary:Object;
      
      protected var _spriteAnchor:Sprite;
      
      protected var _indicator:Array;
      
      public function ItemCompare()
      {
         var diff:MovieClip = null;
         this._spriteAnchor = new Sprite();
         super();
         this._spriteAnchor.x -= 130;
         this._spriteAnchor.y += 120;
         this._indicator = [this.diff_1,this.diff_2,this.diff_3,this.diff_4,this.diff_5,this.diff_6,this.diff_7,this.diff_8];
         diff = MovieClip(this.getChildByName("diff_9"));
         if(diff)
         {
            this._indicator.public::push(diff);
         }
         var i:int = 0;
         while(i < this._indicator.length)
         {
            diff = this._indicator[i];
            diff.visible = false;
            diff.background.visible = false;
            i++;
         }
      }
      
      public static function setItemsMaxValues(limits:Array) : void
      {
         var i:int = 0;
         while(i < limits.length)
         {
            _propsMax[limits[i].icon_id] = limits[i].max;
            _propsMin[limits[i].icon_id] = limits[i].min;
            _propsDir[limits[i].icon_id] = limits[i].direction;
            i++;
         }
      }
      
      // A card property a modifier of another property changes: a weapon's aim-time roll (102,
      // 2070..2074) scales its aim time (133, parameters/aim_time) since weapon-rolls-1.
      private static const MODIFIED_BY:Object = {133:102};

      // Whether a modifier of `modifierProp` changes the card property `prop`.
      private static function modifies(modifierProp:int, prop:int) : Boolean
      {
         return modifierProp == prop || MODIFIED_BY[prop] === modifierProp;
      }

      public static function setPropsDescriptor(descriptor:PropertiesDescriptor) : void
      {
         trace("setPropsDescriptor::",descriptor);
         _propsDescriptor = descriptor;
      }
      
      public function fillModifiers(item:Object) : void
      {
         var modifiers:Array = item.modifiers;
         if(!modifiers)
         {
            return;
         }
         var combined:Object = new Object();
         var propTotals:Object = new Object();
         var i:uint = 0;
         var props:Object;
         var prop:Object;
         var propId:int;
         var propValue:Number;
         while(i < modifiers.length)
         {
            props = modifiers[i].props;
            for each(prop in props)
            {
               propId = prop.prop_id;
               if(propTotals[propId] == undefined)
               {
                  propTotals[propId] = {
                     prop_id:propId,
                     prop_value:0
                  };
               }
               propValue = Number(prop.prop_value);
               if(propValue != Number(prop.prop_value))
               {
                  propValue = Util.stringToNumber(prop.prop_value);
               }
               propTotals[propId].prop_value += propValue;
            }
            i++;
         }
         combined.props = propTotals;
         this.setModifiers(combined);
      }
      
      public function setModifiers(modifiers:Object) : void
      {
         var count:uint = this._indicator.length;
         var prop:Object;
         var i:uint;
         var diff:MovieClip;
         var description:Object;
         var baseValue:Number;
         var upgradeValue:Number;
         if(modifiers)
         {
            for each(prop in modifiers.props)
            {
               i = 0;
               while(i < count)
               {
                  diff = this._indicator[i];
                  if(diff.id == prop.prop_id)
                  {
                     description = _propsDescriptor.getPropertyDescription(prop.prop_id);
                     baseValue = diff.indicator.value - diff.indicator.extra;
                     upgradeValue = description.modifier_type == ModifierType.ADD ? prop.prop_value : baseValue * prop.prop_value;
                     trace("_diff.indicator.value::",description.modifier_type,baseValue,diff.indicator.value,prop.prop_value,upgradeValue);
                     diff.indicator.upgrade = upgradeValue;
                     break;
                  }
                  i++;
               }
            }
         }
         else
         {
            i = 0;
            while(i < count)
            {
               diff = this._indicator[i];
               diff.indicator.upgrade = 0;
               i++;
            }
         }
      }
      
      public function setPrimaryWeapon(item:Object) : void
      {
         _primary = item;
         this.updateData();
      }
      
      public function getPrimaryData() : Object
      {
         return this._primary;
      }

      // The item this card's is compared with: none beside itself.
      public function getSecondaryData() : Object { return this._secondary != this._primary ? this._secondary : null; }

      public function refreshComparison() : void { updateData(); }
      
      // The item to compare with is kept as it is given, and whether it is the card's own item
      // is asked when the card is drawn (getSecondaryData). It was asked here, of the item the
      // card held before: after a hover over the equipped item itself, the next item's card was
      // given that item to compare with, refused it as its own, and stood alone (no colours, no
      // dash for a row only the other has, a shotgun's rounds all listed).
      public function setSecondaryWeapon(item:Object) : void
      {
         if(this._secondary != item)
         {
            _secondary = item;
            this.updateData();
         }
      }
      
      public function setIcon(icon:Sprite) : void
      {
         if(this._spriteAnchor.numChildren)
         {
            this._spriteAnchor.removeChildAt(0);
         }
         if(this._primary.dragIcon is Icon)
         {
            this._spriteAnchor.addChild(this._primary.dragIcon.clone());
         }
      }
      
      protected function updateData() : void
      {
         if(!_propsDescriptor)
         {
            trace("!props_descriptor::");
            return;
         }
         if(!this._primary)
         {
            return;
         }
         trace("ItemCompare::updateData");
         var medical:Boolean = this._primary.category == ItemCategory.MEDKITS;
         var weaponDetails:Boolean = WeaponDetails.read(this._primary) != null;
         // A round's stats are all detail rows (AmmoDetails): no bars.
         var ammoDetails:Boolean = AmmoDetails.read(this._primary) != null;
         var index:int = 0;
         var _diff:MovieClip = null;
         var propDescription:Object = null;
         var indicators:uint = this._indicator.length;
         var modifier_value:Number = 0;
         // Held Alt over a weapon, a medicine, a grenade or a trap: the stats as a match has them, with the kit and the skills.
         var effects:MatchEffects = LocalPlayerProfile.heldMatchEffects(this._primary);
         // The item's own base value of the property last read (initPropValues).
         var base_value:Number = 0;
         function initPropValues(baseProperty:Object, owner:Object):void
         {
            modifier_value = 0;
            var total:Number = 0;
            var modifierType:int = 0;
            var modifierEntry:Object;
            var modifierProp:Object;
            var propValue:Number;
            var modifierDescription:Object;
            var modifierList:Object = owner.modifiers;
            if(modifierList)
            {
               for each(modifierEntry in modifierList)
               {
                  for each(modifierProp in modifierEntry.props)
                  {
                     if(modifies(modifierProp.prop_id,propDescription.prop_id))
                     {
                        propValue = Number(modifierProp.prop_value);
                        if(propValue != Number(modifierProp.prop_value))
                        {
                           propValue = Util.stringToNumber(String(modifierProp.prop_value));
                        }
                        modifier_value += propValue;
                        modifierDescription = _propsDescriptor.getPropertyDescription(modifierProp.prop_id);
                        modifierType = modifierDescription.modifier_type;
                     }
                  }
               }
            }
            // The exe sends a line's prop_value as the config's text ("175", ui_desc/props_list):
            // a number before any sum, where "175" + 0 would be "1750". A weapon's damage and armour
            // piercing with its best round (WeaponDetails.bestRoundFactor), whole as the card shows
            // them (Util.getNumberString cuts a fraction's float tail: 34 x 1.2 read 40.79).
            var base:Number = ModificationRolls.value(baseProperty);
            var roundFactor:Number = WeaponDetails.bestRoundFactor(owner,propDescription.prop_id);
            if(roundFactor != 1) base = Math.round(base * roundFactor);
            base_value = base;
            if(modifierType == ModifierType.ADD)
            {
               total = base + modifier_value;
            }
            else
            {
               // The cook's type 2: base * max(0, 1 + sum) (0x358010).
               total = base * Math.max(0,1 + modifier_value);
            }
            if(modifierType == ModifierType.MULTIPLY)
            {
               modifier_value = base * modifier_value;
            }
            _value = effects == null ? total : effects.stat(owner,propDescription.prop_id,total);
         }
         function initIndicatorValues(diffClip:MovieClip, propertyInfo:Object, compareValue:Number, currentValue:Number):void
         {
            _diff = diffClip;
            _diff.visible = true;
            addChild(_diff);
            _diff.indicator.direction = propertyInfo.direction;
            _diff.indicator.maximum = propertyInfo.max_value;
            _diff.indicator.minimum = propertyInfo.min_value;
            _diff.indicator.label = propertyInfo.prop_name;
            _diff.indicator.upgrade = 0;
            _diff.id = propertyInfo.prop_id;
            if(propertyInfo.postfix != undefined)
            {
               _diff.indicator.postfix = propertyInfo.postfix;
            }
            else
            {
               _diff.indicator.postfix = "";
            }
            _diff.indicator.extra = modifier_value;
            _diff.indicator.compare = compareValue;
            _diff.indicator.value = currentValue;
            _diff.indicator.setChange(null,null);
         }
         // What one unit takes of a pocket (docs/notes/one-pocket-a-type.md, "The pocket's units
         // shown"): 1, the RGD-5's 2, a booster's whole pocket; less is better. The first row.
         var depth:int = pocketDepth(this._primary);
         if(depth > 0 && index < indicators)
         {
            modifier_value = 0;
            initIndicatorValues(this._indicator[index],{prop_id:POCKET_DEPTH,prop_name:LocalizationData.getName("st_srp_inv_pocket_units"),
               min_value:0,max_value:PlayerInfo.MAX_POCKET_UNITS,direction:PropertyDirection.LOWER_IS_BETTER},depth,depth);
            if(depth == PlayerInfo.MAX_POCKET_UNITS) _diff.indicator.setChange(LocalizationData.getName("st_srp_inv_pocket_units_whole"),null);
            index++;
         }
         // A grenade, a trap or a booster has no row for a stat only the other item has (a smoke
         // grenade's damage is not 0). Weapons, armour and medicines have: their stats are of
         // one kind, and the two cards' rows face each other (a medkit's disinfection: 0 %).
         var ownOnly:Boolean = depth > 0 && !(medical && depth < PlayerInfo.MAX_POCKET_UNITS);
         for each(var s:Object in fitted(cardRows(ownOnly),ammoDetails,weaponDetails,indicators - index))
         {
            if(ammoDetails || weaponDetails && s.prop_id == 24) continue;
            var _valueObject:Object = s;
            propDescription = _propsDescriptor.getPropertyDescription(_valueObject.prop_id);
            if(!propDescription)
            {
               trace("wrong propDescription",_valueObject.prop_id);
            }
            else
            {
               var _value:Number = 0;
               trace("primaryProps::",propDescription.prop_name,"value::",_valueObject.prop_value);
               if(propDescription.comparable)
               {
                  initPropValues(_valueObject,this._primary);
                  initIndicatorValues(this._indicator[index],propDescription,_value,_value);
                  // While Alt is held, a value its modifications, the kit or the skills change
                  // reads "base -> in a match", the current green when better, red when worse.
                  if(effects != null && MatchEffects.changed(base_value,_value))
                     _diff.indicator.setChange(MatchEffects.change(base_value,_value,2) + _diff.indicator.postfix,
                        MatchEffects.verdict(base_value,_value,propDescription.direction == PropertyDirection.HIGHER_IS_BETTER));
                  index++;
                  if(index == indicators)
                  {
                     break;
                  }
               }
            }
         }
         // Held Alt over a medicine: below its own lines, what the medical skills do while it is
         // active or as it heals (MatchEffects.medicineSkillLines), "100 -> in a match" in %.
         if(effects != null && medical)
         {
            for each(var line:Object in effects.medicineSkillLines(this._primary))
            {
               if(index == indicators) break;
               modifier_value = 0;
               initIndicatorValues(this._indicator[index],{prop_id:-1,prop_name:line.name,postfix:"%",min_value:0,max_value:200,
                  direction:line.higherIsBetter ? PropertyDirection.HIGHER_IS_BETTER : PropertyDirection.LOWER_IS_BETTER},line.after,line.after);
               _diff.indicator.setChange(MatchEffects.change(line.before,line.after,2) + "%",MatchEffects.verdict(line.before,line.after,line.higherIsBetter));
               index++;
            }
         }
         while(index < indicators)
         {
            _diff = this._indicator[index];
            if(_diff.parent == this) removeChild(_diff);
            _diff.visible = false;
            index++;
         }
         if(!this.comparedItem())
         {
            dispatchEvent(new Event(Event.RESIZE));
            return;
         }
         index = 0;
         while(index < indicators)
         {
            _diff = this._indicator[index];
            if(_diff.visible)
            {
               // A row the other item has none of: against itself where the card lists only
               // its own item's (ownOnly), against 0 otherwise.
               _diff.indicator.compare = ownOnly ? _diff.indicator.value : 0;
            }
            index++;
         }
         for each(s in weightLast(this._secondary.properties))
         {
            if(ammoDetails || weaponDetails && s.prop_id == 24) continue;
            _valueObject = s;
            propDescription = _propsDescriptor.getPropertyDescription(_valueObject.prop_id);
            if(!propDescription)
            {
               trace("wrong propDescription",_valueObject.prop_id,propDescription);
            }
            else if(propDescription.comparable)
            {
               initPropValues(_valueObject,this._secondary);
               index = 0;
               while(index < indicators)
               {
                  _diff = this._indicator[index];
                  if(!_diff.visible)
                  {
                     if(ownOnly) break;
                     initIndicatorValues(this._indicator[index],propDescription,_value,0);
                     break;
                  }
                  else if(_diff.id == propDescription.prop_id)
                  {
                     _diff.indicator.compare = _value;
                     _diff.indicator.extra = 0;
                     break;
                  }
                  index++;
               }
            }
         }
         var theirs:int = pocketDepth(this._secondary);
         index = 0;
         while(theirs > 0 && index < indicators)
         {
            _diff = this._indicator[index];
            if(_diff.visible && _diff.id == POCKET_DEPTH) _diff.indicator.compare = theirs;
            index++;
         }
         dispatchEvent(new Event(Event.RESIZE));
      }

      // The item the card's rows are compared with: the one given, of the card's item's kind (a
      // weapon with a weapon, else of its category), or none.
      private function comparedItem() : Object
      {
         var other:Object = this.getSecondaryData();
         if(!other) return null;
         var kind:int = ItemFactory.getCategory(this._primary.category);
         var same:Boolean = kind == SuperCategory.WEAPONS ? kind == ItemFactory.getCategory(other.category) : this._primary.category == other.category;
         return same ? other : null;
      }

      // The properties the card's rows are of, in order. Alone, or `ownOnly`: the item's own.
      // Beside a compared item: each property either has, in one order on both cards (the
      // properties of the item that has more of them, of the lower dictionary id of two with as
      // many, then the other's own), a property the card's item has none of with the value 0.
      // The weight last.
      private function cardRows(ownOnly:Boolean) : Array
      {
         var other:Object = this.comparedItem();
         if(ownOnly || other == null) return weightLast(this._primary.properties);
         var own:Array = [];
         var theirs:Array = [];
         var prop:Object;
         var mine:Object = {};
         for each(prop in this._primary.properties)
         {
            own.push(prop);
            if(mine[prop.prop_id] == undefined) mine[prop.prop_id] = prop;
         }
         for each(prop in other.properties) theirs.push(prop);
         var otherLeads:Boolean = theirs.length > own.length || theirs.length == own.length && int(other.dictId) < int(this._primary.dictId);
         var rows:Array = [];
         var seen:Object = {};
         for each(prop in otherLeads ? theirs.concat(own) : own.concat(theirs))
         {
            if(seen[prop.prop_id]) continue;
            seen[prop.prop_id] = true;
            rows.push(mine[prop.prop_id] != undefined ? mine[prop.prop_id] : {prop_id:prop.prop_id,prop_value:0});
         }
         return weightLast(rows);
      }

      // Of `rows`, those the card draws (the comparable ones) and has `room` for: with more, the
      // last one (the weight) is kept and the ones before it go.
      private function fitted(rows:Array, ammoDetails:Boolean, weaponDetails:Boolean, room:int) : Array
      {
         var drawn:Array = [];
         for each(var prop:Object in rows)
         {
            if(ammoDetails || weaponDetails && prop.prop_id == 24) continue;
            var description:Object = _propsDescriptor.getPropertyDescription(prop.prop_id);
            if(description && description.comparable) drawn.push(prop);
         }
         if(room > 0 && drawn.length > room) drawn.splice(room - 1,drawn.length - room);
         return drawn;
      }

      // An item's properties in the card's order: its own, the weight last. The weight is the property named as the kit's weight (the exe hands the lobby the
      // properties' names, not their keys).
      private function weightLast(properties:Object) : Array
      {
         var rows:Array = [];
         var weight:Array = [];
         var name:String = LocalizationData.getName("equipment_weight");
         for each(var prop:Object in properties)
         {
            (_propsDescriptor.getPropertyDescription(prop.prop_id).prop_name == name ? weight : rows).push(prop);
         }
         return rows.concat(weight);
      }

      // The card row of a pocket item's units: the id of the property that carries
      // parameters/pocket_units (content-edit pocket-units-1).
      private static const POCKET_DEPTH:int = 166;

      // The pocket units one of `item` takes: all of them (MAX_POCKET_UNITS) for an item that
      // fills a pocket; 0 for an item that goes in no pocket.
      private static function pocketDepth(item:Object) : int
      {
         if(item == null || item.category != ItemCategory.MEDKITS && item.category != ItemCategory.BOOBY_TRAPS && item.category != ItemCategory.GRENADES)
            return 0;
         if(item.quickSlotItemSize == QuickSlotSize.WHOLE) return PlayerInfo.MAX_POCKET_UNITS;
         return item.quickSlotItemSize == QuickSlotSize.PAIR ? 2 : 1;
      }
   }
}
