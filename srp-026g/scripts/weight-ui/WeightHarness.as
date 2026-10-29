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
  private function texts(language:String):Object {var t:Object={btn_friends:"Friends"};for(var k:String in HarnessText.DATA[language])t[k]=HarnessText.DATA[language][k];return t;}
  private function label(s:String,x:Number,y:Number):void {var t:TextField=new TextField();t.defaultTextFormat=new TextFormat("Arial",18,0xBCCBCB);t.text=s;t.x=x;t.y=y;t.width=800;t.height=40;addChild(t);}
  private function loaded(e:Event):void {
   try {
    domain=loader.contentLoaderInfo.applicationDomain;
    var Scene:Object=domain.getDefinition("MainScene");Scene.stage=this;
    var css:StyleSheet=new StyleSheet();css.setStyle("highlight",{color:"#FEE2B2"});css.setStyle("red",{color:"#CF4242"});css.setStyle("green",{color:"#7FC24B"});css.setStyle("weight_warning",{color:"#F18A78",display:"inline"});Scene.textStyleSheet=css;
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
    for(var i:int=0;i<4;i++) {
     var w:Object=new Widget();w.x=40+i*225;w.y=620;addChild(w as DisplayObject);widgets.push(w);
     indicators.push(new Indicator(w));
     label(["20 kg, skills and bonuses","25 kg: slowed","30 kg: carry limit","Heavyweight"][i],w.x,575);
    }
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
  // The kit's speed: base x max(0, 1 + weight + skills + bonuses + weapon), Bear Arms
  // leaving a weapon's penalty out (WeightEffects, SpeedParts).
  private function checkSpeed():void {
   require(!Effects.parse(-.1,.75,0).knowsSpeed && Effects.parse(-.1,.75,0).speedText=="","an old exe gives no speed");
   require(!Effects.parse(-.1,.75,0,.03,7).knowsSpeed,"invalid Bear Arms state refused");
   var e:Object=Effects.parse(-.1,.75,0,.03,0).withItems(.04,-.05);
   require(Math.abs(e.speed-.92)<1e-9,"92% = 1 - 10% + 3% + 4% - 5%: "+e.speed);
   require(e.speedText.indexOf("92%")>=0,"speed value: "+e.speedText);
   var rows:Array=e.rows();
   require(rows.length==6,"total, four parts and sprint stamina");
   require(rows[0].label.indexOf("Movement speed")>=0 && rows[0].label.indexOf("92%")>=0,"total row: "+rows[0].label);
   require(rows[1].label.indexOf("Weight")>=0 && rows[1].label.indexOf("-10%")>=0,"weight part: "+rows[1].label);
   require(rows[2].label.indexOf("Bonuses")>=0 && rows[2].label.indexOf("+4%")>=0,"bonuses part: "+rows[2].label);
   require(rows[3].label.indexOf("Skills")>=0 && rows[3].label.indexOf("+3%")>=0,"skills part: "+rows[3].label);
   require(rows[4].label.indexOf("Equipment")>=0 && rows[4].label.indexOf("-5%")>=0,"equipment part: "+rows[4].label);
   require(Math.abs(Effects.parse(0,0,0,0,1).withItems(0,-.05).speed-1)<1e-9,"Bear Arms leaves a weapon's penalty out");
   require(Math.abs(Effects.parse(0,0,0,0,1).withItems(0,.03).speed-1.03)<1e-9,"Bear Arms keeps a weapon's bonus");
   require(Effects.parse(-.2,1.5,0,0,0).withItems(-1,0).speed==0,"never below zero");
   var fast:Object=Effects.parse(0,0,0,.03,0).withItems(.04,0);
   indicators[0].update(20,30,fast);
   var field:TextField=speedField(0);
   require(field.text.replace(/\s+$/,"")=="107%","speed beside the weight: "+field.text);
   require(speedIcon(0).bitmapData===Icons.getIcon(11).bitmap.bitmapData && speedIcon(0).x+speedIcon(0).width<=field.x+1,"the game's speed icon before it");
   require(field.getTextFormat(field.text.indexOf("107"),field.text.indexOf("107")+1).color==0x7FC24B,"faster than base: green");
   require((field.numLines==1 || field.getLineText(1)=="") && field.x+field.width<=0,"one line, left of the weight: x "+field.x+" w "+field.width+" lines "+field.numLines+" text "+field.text);
   require(widgets[0].textFieldValue.text.indexOf("%")<0,"the weight value keeps its own line");
   indicators[0].update(25,30,Effects.parse(-.1,.75,0,0,0).withItems(0,0));
   require(speedField(0).text.indexOf("90%")>=0 && speedField(0).getTextFormat(speedField(0).text.indexOf("90"),speedField(0).text.indexOf("90")+1).color==0xF18A78,"slower than base: coral");
   indicators[0].update(20,30,Effects.parse(0,0,0));
   require(speedField(0).text=="" && speedIcon(0).bitmapData==null,"an old exe shows no speed");
   indicators[0].update(20,30,fast);
  }
  private function speedIcon(i:int):Bitmap {
   for(var c:int=0;c<widgets[i].numChildren;c++){var b:Bitmap=widgets[i].getChildAt(c) as Bitmap;if(b!=null)return b;}
   throw new Error("no speed icon");
  }
  private function speedField(i:int):TextField {
   for(var c:int=0;c<widgets[i].numChildren;c++){var f:TextField=widgets[i].getChildAt(c) as TextField;if(f!=null && f!==widgets[i].textFieldValue && f!==widgets[i].textFieldLabel)return f;}
   throw new Error("no speed field");
  }
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
     for(var i:int=0;i<4;i++)widgets[i].textFieldLabel.text="Weight";
     indicators[0].update(20,30,Effects.parse(0,0,0));
     checkSpeed();
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
      require((speedField(0).numLines==1 || speedField(0).getLineText(1)=="") && speedField(0).text.indexOf("107")>=0 && speedField(0).text.indexOf("%1$s")<0,"speed line: "+language+" "+speedField(0).text);
      for each(var part:Object in target(0).tipData.effects.rows())require(part.label.indexOf("undefined")<0 && part.label.indexOf("#st_")<0 && part.label.indexOf("%1$s")<0,"localized speed: "+language+" "+part.label);
     }
     Loc.setData(texts("english"));indicators[2].refresh();indicators[0].refresh();show(0);
    }
    if(frame==155) {
     // The speed's parts in the same native card: total, weight, bonuses, skills, equipment, sprint stamina.
     var card:Object=rendered();require(card.alpha==1 && card.numChildren==9,"native card with the speed's six rows: "+card.numChildren);
     var text:String=rowText(card);
     for each(var wanted:String in ["Movement speed: 107%","Weight","Bonuses: +4%","Skills: +3%","Equipment"])require(text.indexOf(wanted)>=0,"tooltip row "+wanted+" in "+text);
     for(var m:int=3;m<9;m++){var line:Object=card.getChildAt(m);require(line.x+line.textFieldLabel.x+line.textFieldLabel.textWidth+4<=card.width && line.y+line.textFieldLabel.textHeight+4<=card.height,"speed row fits");}
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
