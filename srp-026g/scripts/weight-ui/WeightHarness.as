package {
 import flash.display.*;
 import flash.events.*;
 import flash.geom.*;
 import flash.net.*;
 import flash.system.*;
 import flash.text.*;
 public class WeightHarness extends Sprite {
  private var loader:Loader=new Loader();
  private var domain:ApplicationDomain;
  private var widgets:Array=[], indicators:Array=[];
  private var Effects:Object, Tip:Class, Loc:Object, Icons:Object;
  // The inventory's own profile panel, for the speed box's place in it (off the stage's view).
  private var profile:Object;
  private var frame:int=0;
  private var overlay:Sprite=new Sprite();
  public function WeightHarness() {
   stage.align=StageAlign.TOP_LEFT;stage.scaleMode=StageScaleMode.NO_SCALE;
   graphics.beginFill(0x1B201E);graphics.drawRect(0,0,960,720);graphics.endFill();
   loader.contentLoaderInfo.addEventListener(Event.INIT,loaded);
   loader.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR,function(e:UncaughtErrorEvent):void{e.preventDefault();});
   loader.load(new URLRequest("lobby_menu.swf"),new LoaderContext(false,new ApplicationDomain()));
  }
  private function require(ok:Boolean,why:String):void {if(!ok)throw new Error(why);}
  // What the exe gives the movie in `language`: srp's texts (HarnessText, from the dictionaries) and the stock key it reads.
  private function texts(language:String):Object {var t:Object={btn_friends:"Friends"};for(var k:String in HarnessText.DATA[language])t[k]=HarnessText.DATA[language][k];for(k in HarnessText.STOCK[language])t[k]=HarnessText.STOCK[language][k];return t;}
  private function label(s:String,x:Number,y:Number):void {var t:TextField=new TextField();t.defaultTextFormat=new TextFormat("Arial",18,0xBCCBCB);t.text=s;t.x=x;t.y=y;t.width=800;t.height=40;addChild(t);}
  private function loaded(e:Event):void {
   try {
    domain=loader.contentLoaderInfo.applicationDomain;
    var Scene:Object=domain.getDefinition("MainScene");Scene.stage=this;
    var css:StyleSheet=new StyleSheet();css.setStyle("highlight",{color:"#FEE2B2"});css.setStyle("red",{color:"#CF4242"});css.setStyle("green",{color:"#7FC24B"});css.setStyle("white",{color:"#BCCBCB"});css.setStyle("weight_warning",{color:"#F18A78",display:"inline"});Scene.textStyleSheet=css;
    Effects=domain.getDefinition("survarium.player.WeightEffects");
    var Atlas:Class=domain.getDefinition("LobbyMenu_ModifierIcons") as Class;
    var bitmap:Bitmap=new Atlas();
    var Container:Class=domain.getDefinition("survarium.data.IconContainer") as Class;
    Icons=new Container("modifiers",38,38,bitmap.bitmapData,true,0,0,0,false);
    Tip=domain.getDefinition("survarium.player.WeightTipRenderer") as Class;
    Loc=domain.getDefinition("survarium.data.LocalizationData");Loc.setData(texts("english"));
    checkWeaponMovement();
    checkShopMovement();
    var Tool:Object=domain.getDefinition("scaleform.clik.controls.ToolTip");Tool.tipContainer=overlay;
    var Widget:Class=domain.getDefinition("panel_fla.WeightContainer_44") as Class;
    var Indicator:Class=domain.getDefinition("survarium.player.WeightIndicator") as Class;
    label("Coral slowdown warning — native game tooltip",30,24);
    // Two a row: each weight with its speed box (a frame wide) to its left.
    for(var i:int=0;i<4;i++) {
     var w:Object=new Widget();w.x=240+i%2*460;w.y=530+int(i/2)*100;addChild(w as DisplayObject);widgets.push(w);
     indicators.push(new Indicator(w));
     label(["20 kg, skills and bonuses","25 kg: slowed","30 kg: carry limit","Heavyweight"][i],w.x-200,w.y-40);
    }
    var Profile:Class=domain.getDefinition("SrvLocalPlayerProfile") as Class;
    profile=new Profile();profile.x=-2000;
    profile.setAmmoShare(domain.getDefinition("survarium.ammo.AmmoShareLite").getInstance());
    addChild(profile as DisplayObject);stage.invalidate();
    addChild(overlay);
    addEventListener(Event.ENTER_FRAME,tick);
   } catch(e:Error){finish("setup: "+e+" "+e.getStackTrace());}
  }
  private function target(i:int):Object {return widgets[i].getChildAt(widgets[i].numChildren-1).getChildAt(0);}
  private function checkWeaponMovement():void {
   var Data:Class=domain.getDefinition("survarium.data.ItemData") as Class;
   var Movement:Object=domain.getDefinition("survarium.item.WeaponMovement");
   var Props:Class=domain.getDefinition("survarium.data.PropertiesDescriptor") as Class;
   var props:Object=new Props("properties");
   props.setPropertiesDictionary([{prop_id:105,prop_name:"Movement speed",modifier_type:1,direction:1}]);
   var Factory:Class=domain.getDefinition("survarium.item.ItemFactory") as Class;
   var factory:Object=new Factory();
   for each(var amount:Number in [-.05,0,.05]) {
    var shared:Array=[{prop_id:15,prop_value:"7"},{prop_id:105,prop_value:amount.toString()}];
    var defaults:Array=[];
    for(var instance:int=0;instance<2;instance++) {
     var item:Object=new Data();item.isWeapon=true;item.properties=shared;item.modifiersDefault=defaults;
     Movement.expose(item);Movement.expose(item);
     require(item.properties.length==1 && item.properties[0].prop_id==15,"movement leaves the weapon stat table; weight stays");
     require(shared.length==2 && defaults.length==0,"shared descriptors remain unchanged");
     require(item.modifiers.length==(amount==0?0:1),"one built-in row per nonzero modifier, no duplicate cards");
     if(amount!=0) {
      var mod:Object=item.modifiers[0];factory.fillModifierDescription(mod);
      require(mod.isDefault && mod.state==0 && mod.icon==11,"built-in bonus with native speed icon, never critical damage");
      require(mod.textValue.indexOf(amount<0?"-5%":"+5%")>=0,"exact signed weapon speed");
      require(amount>=0 || mod.textValue.indexOf("red")>=0,"penalty uses red");
     }
    }
   }
   var armour:Object=new Data();armour.isWeapon=false;armour.properties=shared;armour.modifiersDefault=defaults;
   Movement.expose(armour);require(armour.properties===shared,"armour properties are unaffected");
  }
  private function checkShopMovement():void {
   var Container:Class=domain.getDefinition("survarium.data.IconContainer") as Class;
   new Container("weapons",1,1,new BitmapData(1,1,true,0),false,1,0,0,false);
   var Descriptor:Class=domain.getDefinition("survarium.data.ItemDescriptor") as Class;
   var descriptor:Object=new Descriptor("weapon-shop-check");
   var Ids:Object=domain.getDefinition("survarium.constants.ItemDictId");
   var golden:int=Ids.GOLDEN_WEAPONS[0];
   var specs:Array=[[2229,-.05],[363,0],[9000,.05],[golden,.05]];
   var descriptors:Array=[],offers:Array=[];
   for each(var spec:Array in specs) {
    descriptors.push({dictId:spec[0],category:16,name:"Weapon "+spec[0],level:15,
     icon_row:1,icon_column:1,double_handed:true,modifiers:[],
     item_properties:[{prop_id:15,prop_value:"7"},{prop_id:105,prop_value:String(spec[1])}]});
    offers.push({dictId:spec[0],faction_id:1,level:0,cost:100,condition_or_stack:10000});
   }
   descriptor.setItemsDictionary(descriptors);
   var Shop:Class=domain.getDefinition("survarium.controls.GangShop") as Class;
   var shop:Object=new Shop();shop.registerItemDescriptor(descriptor);
   var check:Function=function(item:Object):Boolean {
    var expected:Number=0;
    for each(var entry:Array in specs) if(entry[0]==item.dictId) expected=entry[1];
    var movement:Array=[];
    for each(var mod:Object in item.modifiers)
     for each(var prop:Object in mod.props) if(prop.prop_id==105) movement.push(mod);
    require(movement.length==(expected==0?0:1),"shop exposes movement for weapon "+item.dictId);
    if(expected!=0) require(movement[0].isDefault && movement[0].icon==11 && movement[0].props[0].prop_value==expected,"shop preserves native weapon modifier");
    for each(var stat:Object in item.properties) require(stat.prop_id!=105,"shop movement is in bonuses, not main stats");
    if(item.dictId==golden) require(item.modifiers.length==2,"golden shop weapon retains silver bonus alongside speed");
    return false;
   };
   // Exercise the actual shop constructor and the reconnect price-list path,
   // which are separate from ItemFactory and InventoryList.
   shop.setupShopItems(offers);shop.forEachItem(check);
   shop.setupShopItems(offers);shop.forEachItem(check);
  }
  // The kit's speed, a second indicator left of the weight with its own tooltip:
  // base x max(0, 1 + weight + skills + bonuses + weapon), Bear Arms leaving a
  // weapon's penalty out (WeightEffects, SpeedParts, SpeedExplanation, SpeedIndicator).
  private function checkSpeed():void {
   require(Effects.parse(-.1,.75,0).speedExplanation==null,"an old exe gives no speed");
   require(Effects.parse(-.1,.75,0,.03,7).speedExplanation==null,"invalid Bear Arms state refused");
   var e:Object=Effects.parse(-.1,.75,0,.03,0).withItems(kit(.04,.05,-.05));
   var x:Object=e.speedExplanation;
   require(Math.abs(x.total-.97)<1e-9,"97% = 1 - 10% + 3% + 4% + 5% (Tornado) - 5%: "+x.total);
   require(x.value.indexOf("97%")>=0 && x.title.indexOf("97%")>=0 && x.titleValue=="97%" && x.titleStyle=="red","value and heading: "+x.value+" "+x.title);
   var rows:Array=x.rows();
   require(rows.length==5,"the speed's five parts");
   // Each part coloured by its effect, as the item cards: green faster, red slower, white at 0.
   for each(var want:Array in [[0,"Weight","<red>-10%</red>"],[1,"Bonuses","<green>+4%</green>"],[2,"Boosters","<green>+5%</green>"],[3,"Skills","<green>+3%</green>"],[4,"Equipment","<red>-5%</red>"]])
    require(rows[want[0]].label.indexOf(want[1])>=0 && rows[want[0]].label.indexOf(want[2])>=0,want[1]+" part: "+rows[want[0]].label);
   require(Effects.parse(0,0,0,0,0).withItems(kit(0,0,0)).speedExplanation.rows()[0].label.indexOf("<white>0%</white>")>=0,"no effect: white");
   require(Effects.parse(0,0,0,0,0).withItems(kit(0,0,0)).speedExplanation.titleStyle=="white" && Effects.parse(0,0,0,.03,0).withItems(kit(0,0,0)).speedExplanation.titleStyle=="green","heading total: white at 100%, green above");
   require(e.rows().length==2 && e.rows()[0].label.indexOf("-10%")>=0 && e.rows()[0].label.indexOf("Skills")<0 && e.titleValue==null,"the weight's tooltip stays the weight's");
   require(Math.abs(Effects.parse(0,0,0,0,1).withItems(kit(0,0,-.05)).speedExplanation.total-1)<1e-9,"Bear Arms leaves a weapon's penalty out");
   require(Math.abs(Effects.parse(0,0,0,0,1).withItems(kit(0,0,.03)).speedExplanation.total-1.03)<1e-9,"Bear Arms keeps a weapon's bonus");
   require(Effects.parse(-.2,1.5,0,0,0).withItems(kit(-1,0,0)).speedExplanation.total==0,"never below zero");
   var fast:Object=Effects.parse(0,0,0,.03,0).withItems(kit(.04,0,0));
   indicators[0].update(20,30,fast);
   var speed:Object=speedWidget(0),weight:Object=widgets[0];
   var label:TextField=speed.textFieldLabel,value:TextField=speed.textFieldValue;
   require(speedBox(0).visible && label.text.replace(/\s+$/,"")=="Speed","its own word: "+label.text);
   require(value.text.replace(/\s+$/,"")=="107%" && value.getTextFormat(0,1).color==0x7FC24B,"107%, green above the base: "+value.text);
   require(speed is (domain.getDefinition("panel_fla.WeightContainer_44") as Class) && !speed.progress_indicator.visible,"the weight's own symbol, without the weight's bar");
   for each(var pair:Array in [[label,weight.textFieldLabel],[value,weight.textFieldValue]]) {
    var f:TextFormat=pair[0].getTextFormat(0,1),g:TextFormat=pair[1].getTextFormat(0,1);
    require(f.font==g.font && f.size==g.size && f.bold==g.bold && pair[0].embedFonts==pair[1].embedFonts && pair[0].filters.length==pair[1].filters.length,"the weight's text style: "+pair[0].name);
    require(pair[0].localToGlobal(new Point()).y==pair[1].localToGlobal(new Point()).y,"on the weight's baseline: "+pair[0].name);
   }
   require(label.getTextFormat(0,1).color==weight.textFieldLabel.getTextFormat(0,1).color,"the weight label's colour");
   // Its frame is the weight's, one frame width left of the weight's: the same place on it.
   var frame:Bitmap=speedBox(0).getChildAt(0) as Bitmap;
   require(frame.bitmapData.width==177 && frame.bitmapData.height==47,"the weight's frame");
   var at:Point=speed.localToGlobal(new Point()),home:Point=weight.localToGlobal(new Point());
   require(at.x==home.x-177 && at.y==home.y,"beside the weight, on its row: "+at+" "+home);
   var hover:Rectangle=speedTip(0).getBounds(speedBox(0)),words:Rectangle=speed.getBounds(speedBox(0));
   require(hover.containsRect(words) && hover.width>=150,"its tooltip covers the whole box: "+hover);
   require(weight.textFieldValue.text.indexOf("%")<0,"the weight keeps its own value");
   indicators[0].update(25,30,Effects.parse(-.1,.75,0,0,0).withItems(kit(0,0,0)));
   require(speedWidget(0).textFieldValue.text.indexOf("90%")>=0 && speedWidget(0).textFieldValue.getTextFormat(0,1).color==0xF18A78,"slower than base: coral");
   indicators[0].update(20,30,Effects.parse(0,0,0));
   require(!speedBox(0).visible,"an old exe shows no speed");
   indicators[0].update(20,30,fast);
  }
  // The speed's box: right under its weight in the weight's parent; its frame, its copy of the weight's symbol, its tooltip.
  private function speedBox(i:int):Sprite {var p:DisplayObjectContainer=widgets[i].parent;var b:Sprite=p.getChildAt(p.getChildIndex(widgets[i])-1) as Sprite;if(b==null||b.name!="speed")throw new Error("no speed box");return b;}
  // The profile's sums of the kit's items (KitItems).
  private function kit(speedBonuses:Number,speedBoosters:Number,weapon:Number,limitBonuses:Number=0,limitBoosters:Number=0):Object {
   var k:Object=new (domain.getDefinition("survarium.player.KitItems") as Class)();
   k.speedBonuses=speedBonuses;k.speedBoosters=speedBoosters;k.weapon=weapon;k.limitBonuses=limitBonuses;k.limitBoosters=limitBoosters;return k;
  }
  // The weight's tooltip: what raises the carry limit, by source, adding up to it; Packrat when the kit has it.
  private function checkLimit():void {
   var e:Object=Effects.parse(-.1,.75,0,.03,0,44,30,4,-.15).withItems(kit(0,0,0,5,5));
   var rows:Array=e.rows();
   require(rows.length==7,"weight rows, base, skills, bonuses, boosters, Packrat: "+rows.length);
   for each(var want:Array in [[2,"Base limit","30 kg"],[3,"Skills","<green>+4 kg</green>"],[4,"Bonuses","<green>+5 kg</green>"],[5,"Boosters","<green>+5 kg</green>"],[6,"Packrat","<green>-15% weight penalty</green>"]])
    require(rows[want[0]].label.indexOf(want[1])>=0 && rows[want[0]].label.indexOf(want[2])>=0,want[1]+" row: "+rows[want[0]].label);
   require(rows[3].icon==12 && rows[6].icon==11,"the weight-limit and speed icons");
   var rest:Array=Effects.parse(0,0,0,0,0,32,30,0,0).withItems(kit(0,0,0)).rows();
   require(rest.length==4 && rest[3].label.indexOf("Equipment")>=0 && rest[3].label.indexOf("+2 kg")>=0,"what the exe's limit has beyond the parts: "+rest[rest.length-1].label);
   require(Effects.parse(0,0,0,0,0,30,30,0,0).withItems(kit(0,0,0)).rows().length==3,"only the base when nothing raises it");
   require(Effects.parse(-.1,.75,0).rows().length==2,"an older exe: the weight rows only");
  }
  private function speedWidget(i:int):Object {return speedBox(i).getChildAt(1);}
  private function speedTip(i:int):Object {return speedTipOf(speedBox(i));}
  // In the inventory's profile, in `language`: the speed's box apart from the Autobuy box's text, the boots slot
  // and the weight's frame, inside the panel; its word apart from its value.
  private function checkProfileRow(language:String):void {
   var p:DisplayObjectContainer=profile as DisplayObjectContainer;
   // The profile took the texts before this language (LocalizationData.addCallback calls it once then):
   // its next kit update puts the speed's word in.
   profile.updateWeight(25,30,Effects.parse(-.1,.75,0,.03,0));
   var box:Sprite=p.getChildByName("speed") as Sprite;require(box!=null && box.visible,"the profile's speed box: "+language);
   var area:Rectangle=speedTipOf(box).getBounds(p);
   var autobuy:Object=profile.checkbox_autobuy;autobuy.validateNow();var text:TextField=autobuy.textField;
   require(text.text==HarnessText.STOCK[language].autobuy_button_label,"Autobuy's text: "+language+" "+text.text);
   var autobuyRight:Number=text.localToGlobal(new Point(text.textWidth+4,0)).x-p.localToGlobal(new Point()).x;
   require(autobuyRight<=area.left,"apart from Autobuy: "+language+" "+autobuyRight+" "+area);
   require(!area.intersects(profile.profileItems.slotFoot.getBounds(p)),"apart from the boots: "+language);
   require(area.right<=profile.playerWeight.x && area.left>=0,"left of the weight, inside the panel: "+language+" "+area);
   var words:Object=box.getChildAt(1),sl:TextField=words.textFieldLabel,sv:TextField=words.textFieldValue;
   require(sl.text==HarnessText.DATA[language].st_srp_speed_label,"translated word: "+language);
   require(sl.x+2+sl.textWidth+6<=sv.x+sv.width-2-sv.textWidth,"its word apart from its value: "+language+" "+sl.text+" "+sv.text);
   trace("SPEED ROW "+language+": Autobuy text ends at "+autobuyRight.toFixed(1)+", box "+area+", word "+sl.textWidth.toFixed(1)+" + value "+sv.textWidth.toFixed(1)+" of "+(sv.x+sv.width-sl.x).toFixed(1));
  }
  private function speedTipOf(box:Sprite):DisplayObject {return (box.getChildAt(2) as Sprite).getChildAt(0);}
  private function show(i:int):void {target(i).dispatchEvent(new MouseEvent(MouseEvent.MOUSE_OVER));}
  private function hide(i:int):void {target(i).dispatchEvent(new MouseEvent(MouseEvent.ROLL_OUT));}
  private function rendered():Object {
   require(overlay.numChildren==1,"exactly one tooltip");
   var r:Object=overlay.getChildAt(0);require(r is Tip,"existing game renderer");return r;
  }
  private function rowText(r:Object):String {
   var result:String="";
   for(var i:int=3;i<r.numChildren;i++)result+=r.getChildAt(i).textFieldLabel.text+"\n";
   return result;
  }
  private function tick(e:Event):void {
   frame++;
   try {
    if(frame==20) {
     require(Effects.parse(NaN,NaN,NaN)==null,"old callback remains unknown");
     require(Effects.parse(-.1,.75,99)==null,"invalid sprint state refused");
     // The paper doll's ammo control is the lobby's; the profile's weight callback refreshes it.
     profile.paperDoll.ammoControl=new (domain.getDefinition("survarium.ammo.AmmoShareControll") as Class)();
     profile.updateWeight(25,30,Effects.parse(-.1,.75,0,.03,0));
     for(var i:int=0;i<4;i++)widgets[i].textFieldLabel.text="Weight";
     indicators[0].update(20,30,Effects.parse(0,0,0));
     checkSpeed();
     checkLimit();
     indicators[1].update(25,30,Effects.parse(-.1,.75,0));
     indicators[2].update(30,30,Effects.parse(-.2,1.5,1));
     indicators[3].update(25,30,Effects.parse(0,.75,2));
     require(widgets[1].textFieldValue.getTextFormat(0,1).color==0xF18A78,"coral styling");
     require(widgets[1].textFieldValue.numLines==1,"weight value stays on one line");
     require(widgets[2].textFieldValue.getTextFormat(0,1).color==0xCF4242,"red at carry limit");
     require(target(1).tipData.effects.rows()[0].label.indexOf("-10%")>=0 && target(1).tipData.effects.rows()[1].label.indexOf("+75%")>=0,"exact penalties");
     require(target(3).tipData.effects.rows().length==2 && target(3).tipData.effects.rows()[1].icon==11,"Heavyweight has no hypothetical sprint cost");
     var detached:Object=new Tip();detached.data=target(1).tipData;
     require(detached.alpha==0,"native card stays hidden before its timeline initializes");
     show(1);
    }
    if(frame==65) {
     var r:Object=rendered();require(r.alpha==1,"initialized native card is visible");require(r.numChildren==5,"native frame, divider and two stat rows");
     var heading:Object=r.getChildAt(2);
     require(heading.y+heading.textField.y>=4,"heading fits inside the native frame");
     require(rowText(r).indexOf("Movement speed")>=0,"speed line");
     var Row:Class=domain.getDefinition("SrvModifierLabeledRenderer") as Class;
     for(var n:int=3;n<5;n++) {
      var row:Object=r.getChildAt(n);require(row is Row,"same row as critical damage");
      require(row.iconContainer.getChildAt(row.iconContainer.numChildren-1).bitmap.bitmapData===Icons.getIcon(n==3?11:16).bitmap.bitmapData,"existing speed and energy icons");
      require(row.x+row.textFieldLabel.x+row.textFieldLabel.textWidth+4<=r.width,"text fits width");
      require(row.y+row.textFieldLabel.textHeight+4<=r.height,"text fits height");
     }
     require(r.x>=0 && r.y>=0 && r.x+r.width<=960 && r.y+r.height<=720,"tip inside screen");
     indicators[1].update(30,30,Effects.parse(-.2,1.5,1));
    }
    if(frame==70) {
     require(rowText(rendered()).indexOf("carry limit")>=0,"open tooltip refreshes when loadout changes");
     hide(1);require(overlay.numChildren==0,"roll out removes tooltip");
     indicators[1].update(25,30,null);require(!target(1).tipEnabled,"unknown effects suppress tooltip");
     indicators[1].update(25,30,Effects.parse(-.1,.75,0));
     show(1);
    }
    if(frame==110) {
     rendered();removeChild(widgets[1]);require(overlay.numChildren==0,"removal cleans tooltip");
     addChildAt(widgets[1],numChildren-1);
     for each(var language:String in HarnessText.LANGUAGES) {
      Loc.setData(texts(language));indicators[2].refresh();
      var title:String=target(2).tipData.effects.title;
      require(title==HarnessText.DATA[language].st_srp_weight_title,"translated title: "+language);
      for each(var entry:Object in target(2).tipData.effects.rows())require(entry.label.indexOf("undefined")<0 && entry.label.indexOf("#st_")<0 && entry.label.indexOf("%1$s")<0,"localized explanation: "+language+" "+entry.label);
      require(target(2).tipData.effects.rows()[0].label.indexOf("-20")>=0,"value in the translated line: "+language);
      indicators[0].refresh();
      var sl:TextField=speedWidget(0).textFieldLabel,sv:TextField=speedWidget(0).textFieldValue;
      require(sl.text==HarnessText.DATA[language].st_srp_speed_label && sv.text.indexOf("107")>=0 && sl.x+2+sl.textWidth+6<=sv.x+sv.width-2-sv.textWidth,"speed item: "+language+" "+sl.text+" "+sv.text);
      checkProfileRow(language);
      require(speedTip(0).tipData.effects.title.indexOf("107%")>=0,"speed heading: "+language);
      for each(var lim:Object in Effects.parse(-.1,.75,0,.03,0,44,30,4,-.15).withItems(kit(0,0,0,5,5)).rows())require(lim.label.indexOf("undefined")<0 && lim.label.indexOf("#")<0 && lim.label.indexOf("%1$s")<0,"localized limit row: "+language+" "+lim.label);
      for each(var part:Object in speedTip(0).tipData.effects.rows())require(part.label.indexOf("undefined")<0 && part.label.indexOf("#st_")<0 && part.label.indexOf("%1$s")<0,"localized speed: "+language+" "+part.label);
     }
     Loc.setData(texts("english"));indicators[2].refresh();indicators[0].refresh();speedTip(0).dispatchEvent(new MouseEvent(MouseEvent.MOUSE_OVER));
    }
    if(frame==155) {
     // The speed's own native card: its total in the heading, then weight, bonuses, skills, equipment.
     var card:Object=rendered();require(card.alpha==1 && card.numChildren==8,"native card with the speed's five rows: "+card.numChildren);
     var head:TextField=card.getChildAt(2).textField;var h:int=head.text.indexOf("107%");
     require(h>=0 && head.getTextFormat(h,h+1).color==0x87A816,"heading total in the rows' green: "+head.text+" "+head.getTextFormat(h,h+1).color.toString(16));
     var text:String=rowText(card);
     for each(var wanted:String in ["Weight","Bonuses: +4%","Boosters","Skills: +3%","Equipment"])require(text.indexOf(wanted)>=0,"tooltip row "+wanted+" in "+text);
     // The colours as drawn: a bonus green, no effect white (the style sheet's).
     var bonus:TextField=card.getChildAt(4).textFieldLabel;var bi:int=bonus.text.indexOf("+4%");
     require(bi>=0 && bonus.getTextFormat(bi,bi+1).color==0x87A816,"a part that speeds up is green, the card rows' green: "+bonus.getTextFormat(bi,bi+1).color.toString(16));
     var slow:TextField=card.getChildAt(3).textFieldLabel;var si:int=slow.text.indexOf("-");
     require(si<0 || slow.getTextFormat(si,si+1).color==0x960F0E,"a part that slows down is red, the card rows' red");
     for(var m:int=3;m<8;m++){var line:Object=card.getChildAt(m);require(line.x+line.textFieldLabel.x+line.textFieldLabel.textWidth+4<=card.width && line.y+line.textFieldLabel.textHeight+4<=card.height,"speed row fits");}
     require(card.x>=0 && card.y>=0 && card.x+card.width<=960 && card.y+card.height<=720,"speed card inside screen");
     finish(null);
    }
   }catch(e:Error){finish("frame "+frame+": "+e+" "+e.getStackTrace());}
  }
  private function finish(error:String):void {
   removeEventListener(Event.ENTER_FRAME,tick);
   trace("WEIGHT RESULT "+(error?"fail: "+error:"pass"));
   if(loaderInfo.parameters.hold!="1")fscommand("quit");
  }
 }
}
