package survarium.messages
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.controls.TileList;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.gfx.TextFieldEx;
   import survarium.Util;
   import survarium.buttons.CheckBoxValue;
   import survarium.controls.ItemRendererBase;
   import survarium.controls.Message;
   import survarium.controls.PlayerInfo;
   import survarium.controls.SilverPrice;
   import survarium.data.LocalizationData;
   import survarium.constants.ItemCondition;
   import survarium.constants.RepairPayment;
   
   public class MessageRepair extends MessageAccept
   {
      
      public var redGlow:Sprite;
      
      public var redGlowBottom:Sprite;
      
      public var background:Sprite;
      
      public var itemSlot:ItemRendererBase;
      
      public var checkBox_1:CheckBoxValue;
      
      public var checkBox_2:CheckBoxValue;
      
      public var checkBox_3:CheckBoxValue;
      
      public var textFieldLabel:TextField;
      
      public var textFieldDamageLabel:TextField;
      
      public var textFieldDamage:TextField;
      
      public var textFieldValue:TextField;
      
      public var textFieldCritical:TextField;
      
      public var textFieldRegular:TextField;
      
      public var textFieldPremium:TextField;
      
      public var textFieldVoucher:TextField;
      
      public var timeToRepair:MovieClip;

      // The price, centred, in place of the pay choices (SilverPrice).
      private var _price:SilverPrice;
      
      public var icon:MovieClip;
      
      public var modifiersList:TileList;
      
      private var repairCost:uint = 0;
      
      private var repairCostGeneral:uint = 0;
      
      private var repairTimeGeneral:uint = 0;
      
      public function MessageRepair()
      {
         super();
      }
      
      override protected function cleanUp(event:Event = null) : void
      {
         super.cleanUp();
         this.itemSlot.data = null;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         var renderer:Class = getDefinitionByName("SrvItemRendererFull") as Class;
         var l:uint;
         var i:uint;
         var modifiersContainer:Sprite;
         var containerScale:Number;
         var nh:uint;
         var backWidth:int;
         var sz:int;
         var pos:int;
         var offset:int;
         var dmg:int;
         var _color:uint;
         var hasModifiers:Boolean;
         var bbHeight:int;
         var bbPos:int;
         itemSlot = new renderer();
         this.itemSlot.x = (this.background.width - this.itemSlot.width) / 2;
         this.itemSlot.y = 42;
         addChildAt(this.itemSlot,numChildren - 3);
         btnAccept.label = LocalizationData.getName("fix");
         tabEnabled = true;
         btnAccept.enabled = false;
         if(_data)
         {
            trace("in:data::",this.textFieldDamageLabel,this.textFieldCritical,this.textFieldRegular,this.textFieldPremium,this.textFieldVoucher);
            this.textFieldDamageLabel.text = LocalizationData.getName("item_damage_state");
            this.textFieldCritical.text = LocalizationData.getName("critical_damages");
            this.textFieldRegular.text = LocalizationData.getName("regular_fix");
            // No quick fix: it was paid in gold, and the game has no gold (the owner, 2026-11-08).
            this.textFieldPremium.visible = false;
            this.checkBox_2.visible = this.checkBox_2.enabled = false;
            // No repair kit either (the owner, 2026-11-08: "premium ticket and repair kit should be
            // removed too"): a repair is paid in silver, its price centred where the choices were
            // ("it should just be the price in the middle in silver"); no choice, so no label for one.
            this.textFieldVoucher.visible = false;
            this.checkBox_3.visible = this.checkBox_3.enabled = false;
            this.textFieldRegular.visible = false;
            this.textFieldLabel.text = LocalizationData.getName("repair_window_label");
            this.itemSlot.data = _data;
            this.redGlow.visible = _data.modifiers.length > 0;
            this.redGlowBottom.visible = this.redGlow.visible;
            this.redGlow.alpha = this.redGlow.visible ? 1 : 0;
            repairCostGeneral = 0;
            repairTimeGeneral = 0;
            l = (_data.modifiers as Array).length;
            for(i = 0; i < l; i++)
            {
               repairCostGeneral += _data.modifiers[i].fix_cost_general;
               repairTimeGeneral += _data.modifiers[i].fix_time_general;
            }
            this.timeToRepair.textField.text = Util.getTimeString(this.repairTimeGeneral);
            this.timeToRepair.visible = this.repairTimeGeneral > 0;
            repairCost = (1 - _data.value / ItemCondition.FULL) * (_data.repairCostRatio * _data.cost) + this.repairCostGeneral;
            trace("MR::init",this.repairCost,_data.value / ItemCondition.FULL,"repairCostGeneral:",this.repairCostGeneral,_data.modifiers,"cost::",_data.cost,_data.repairCostRatio,_data.value);
            this.checkBox_1.value = this.repairCost;
            this.checkBox_1.validateNow();
            this.timeToRepair.x = this.checkBox_1.x + this.checkBox_1.textFieldValue.x + this.checkBox_1.textFieldValue.textWidth + 10;
            this.textFieldDamage.text = _data.value + "%";
            modifiersContainer = new Sprite();
            containerScale = 0.7;
            addChild(modifiersContainer);
            modifiersContainer.x = this.modifiersList.x;
            modifiersContainer.y = this.modifiersList.y;
            modifiersContainer.addChild(this.modifiersList);
            modifiersContainer.scaleX = modifiersContainer.scaleY = containerScale;
            this.modifiersList.x = 0;
            this.modifiersList.y = 0;
            this.modifiersList.dataProvider = new DataProvider(_data.modifiers as Array);
            nh = this.modifiersList.dataProvider.length * this.modifiersList.columnWidth + 5;
            this.modifiersList.width = Math.max(nh,1);
            this.modifiersList.validateNow();
            this.modifiersList.visible = this.modifiersList.dataProvider.length != 0;
            backWidth = this.background.width;
            sz = this.textFieldDamageLabel.textWidth + this.textFieldDamage.textWidth + 8;
            pos = (backWidth - sz) / 2;
            offset = this.textFieldDamageLabel.getLineMetrics(0).x;
            this.textFieldDamageLabel.x = pos - offset;
            this.textFieldDamage.mouseEnabled = false;
            this.timeToRepair.mouseEnabled = false;
            this.timeToRepair.mouseChildren = false;
            this.timeToRepair.y -= 1;
            this.textFieldDamage.x = this.textFieldDamageLabel.x + this.textFieldDamageLabel.textWidth + 8 + offset;
            sz = this.textFieldCritical.textWidth + modifiersContainer.width + 8;
            pos = (backWidth - sz) / 2;
            offset = this.textFieldCritical.getLineMetrics(0).x;
            this.textFieldCritical.x = pos - offset;
            modifiersContainer.x = this.textFieldCritical.x + this.textFieldCritical.textWidth + 8 + offset;
            dmg = _data.value / ItemCondition.FULL * 510;
            _color = Util.getColor(Math.min(510 - dmg,255),Math.min(dmg,255),0);
            this.textFieldDamage.textColor = _color;
            hasModifiers = (_data.modifiers as Array).length > 0;
            trace("hasModifiers",hasModifiers,(_data.modifiers as Array).length);
            this.background.height = hasModifiers ? 413 : 413 - 25;
            this.textFieldCritical.visible = this.modifiersList.visible = hasModifiers;
            offset = hasModifiers ? -8 : 25;
            this.checkBox_1.y -= offset;
            this.timeToRepair.y -= offset;
            bbHeight = 48;
            bbPos = this.background.height - 3;
            btnAccept.y = bbPos + 5 - bbHeight;
            btnCancel.y = btnAccept.y;
            this._price = new SilverPrice(this.checkBox_1,this.background.x + this.background.width / 2);
            this._price.cost = this.repairCost;
            this._price.alpha = this.repairCost <= PlayerInfo.genericMoney ? 1 : 0.5;
            this.timeToRepair.x = 160 - this.timeToRepair.textField.x - this.timeToRepair.textField.textWidth - 1;
         }
         this.checkAcceptButton();
      }
      
      protected function checkAcceptButton() : void
      {
         btnAccept.enabled = this.repairCost <= PlayerInfo.genericMoney;
      }
      
      override protected function onMessageAccept(event:ButtonEvent = null) : void
      {
         if(!btnAccept.enabled)
         {
            return;
         }
         super.onMessageAccept();
         ExternalInterface.call("repair_item",uint(_data.itemId),uint(RepairPayment.REGULAR));
      }
   }
}

